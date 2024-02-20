use async_recursion::async_recursion;
use std::{collections::HashMap, fs::File, io::Write, path::Path};

use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Deserialize, Hash, PartialEq, Eq)]
enum LangEnum {
    KR,
    EN,
    CN,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CategoryResponse {
    pub code: String,
    pub message: String,
    pub data: Vec<CategoryChildResponse>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CategoryChildResponse {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub position: i32,
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    pub titles: HashMap<LangEnum, String>,
    pub children: Vec<CategoryChildResponse>,
    pub modified: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ArticleResponse {
    pub code: String,
    pub message: String,
    pub data: ArticleDataResponse,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ArticleDataResponse {
    pub id: i32,
    pub category_id: i32,
    pub category_titles: HashMap<LangEnum, String>,
    pub status: String,
    pub titles: HashMap<LangEnum, String>,
    pub subtitles: HashMap<LangEnum, String>,
    pub image_url: Option<String>,
    pub attachments: HashMap<LangEnum, Vec<ArticleAattachment>>,
    pub contents: HashMap<LangEnum, String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ArticleAattachment {
    pub id: i32,
    #[serde(rename = "type")]
    pub type_: String,
    pub position: i32,
    pub source_url: String,
    pub thumbnail_url: String,
    pub modified: bool,
    pub status: String,
}

#[derive(Parser, Debug)]
struct Arguments {
    #[arg(short, long, default_value = "true")]
    use_local: bool,
}

async fn read_from_web() -> anyhow::Result<(Vec<String>, Vec<ArticleDataResponse>)> {
    let categories_url = "https://static.dnf-universe.com/categories.json";
    let categories = get_category_response(categories_url).await.unwrap();

    let mut category_names = vec![];
    let mut ko_articles = vec![];

    iterate_children(&categories.data, &mut category_names, &mut ko_articles).await?;

    Ok((category_names, ko_articles))
}

async fn read_from_local() -> anyhow::Result<(Vec<String>, Vec<ArticleDataResponse>)> {
    let category_names = std::fs::read_to_string("crawled_data/category/categories.json")?;
    let category_response: CategoryResponse = serde_json::from_str(&category_names)?;
    let ko_articles_path = Path::new("crawled_data").join("articles");

    let ko_articles = std::fs::read_dir(ko_articles_path)?
        .map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            let file = File::open(path).unwrap();

            let article: ArticleResponse = serde_json::from_reader(file).unwrap();
            article.data
        })
        .collect::<Vec<_>>();

    Ok((
        category_response
            .data
            .iter()
            .map(|child| child.titles[&LangEnum::KR].clone())
            .collect(),
        ko_articles,
    ))
}

async fn post_process(
    ko_articles: &Vec<ArticleDataResponse>,
    category_names: &Vec<String>,
    exclude_categories: &Vec<String>,
) -> anyhow::Result<()> {
    // Post processing
    let ko_articles_body = ko_articles
        .iter()
        .filter_map(|article| {
            if exclude_categories.contains(&article.category_titles[&LangEnum::KR]) {
                None
            } else {
                Some(format!(
                    "```[{}]```\\\n{}\n\n\n\n",
                    article.titles[&LangEnum::KR],
                    article.contents[&LangEnum::KR]
                ))
            }
        })
        .collect::<Vec<_>>();

    let category_names = category_names
        .into_iter()
        .filter(|name| !exclude_categories.contains(name))
        .map(|name| name.clone())
        .collect::<Vec<_>>();

    let category_names_body = category_names.join("\n");
    let ko_articles_body = ko_articles_body.join("\n");

    let final_dir = Path::new("crawled_data").join("final");
    std::fs::create_dir_all(final_dir.clone()).unwrap();

    let mut category_names_file = File::create(final_dir.join("category_names.txt"))?;
    let mut all_articles_file = File::create(final_dir.join("all_articles.md"))?;

    category_names_file.write(category_names_body.as_bytes())?;
    all_articles_file.write(ko_articles_body.as_bytes())?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Arguments::parse();

    let (category_names, ko_articles) = if args.use_local {
        read_from_local().await?
    } else {
        read_from_web().await?
    };

    let exclude_categories = vec!["명예의 전당", "스페셜", "아트던展"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    post_process(&ko_articles, &category_names, &exclude_categories).await?;

    Ok(())
}

#[async_recursion]
async fn iterate_children(
    children: &Vec<CategoryChildResponse>,
    category_names: &mut Vec<String>,
    ko_articles: &mut Vec<ArticleDataResponse>,
) -> anyhow::Result<()> {
    for child in children {
        let child_type = &child.type_;

        if child_type == "ARTICLE" {
            let article = get_article_content(child.id).await?;

            println!(
                "{} - {}",
                article.data.category_titles[&LangEnum::KR],
                article.data.titles[&LangEnum::KR]
            );

            ko_articles.push(article.data);

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        } else if child_type == "CATEGORY" {
            category_names.push(child.titles[&LangEnum::KR].clone());
        }

        if !child.children.is_empty() {
            let _ = iterate_children(&child.children, category_names, ko_articles).await;
        }
    }

    Ok(())
}

async fn get_category_response(url: &str) -> anyhow::Result<CategoryResponse> {
    let body = get_page_content(url).await?;

    let file_path = Path::new("crawled_data")
        .join("category")
        .join("categories.json");

    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

    let mut file = File::create(file_path)?;
    file.write(body.as_bytes())?;

    let category_response: CategoryResponse = serde_json::from_str(&body)?;

    Ok(category_response)
}

async fn get_page_content(url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder().build()?;
    let res = client.get(url).send().await?;
    let body = res.text().await?;

    Ok(body)
}

async fn get_article_content(id: i32) -> anyhow::Result<ArticleResponse> {
    let url = format!("https://www.dnf-universe.com/api/v1/story/{}", id);
    let body = get_page_content(&url).await?;

    let file_path = Path::new("crawled_data")
        .join("articles")
        .join(format!("{}.json", id));

    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

    let mut file = File::create(file_path)?;
    file.write(body.as_bytes())?;

    let article_response: ArticleResponse = serde_json::from_str(&body)?;

    Ok(article_response)
}
