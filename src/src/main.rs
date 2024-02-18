use async_recursion::async_recursion;
use std::{collections::HashMap, fs::File, io::Write, path::Path};

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
    pub image_url: String,
    pub attachments: HashMap<String, Vec<ArticleAattachment>>,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let categories_url = "https://static.dnf-universe.com/categories.json";
    let categories = get_category_response(categories_url).await.unwrap();

    let mut category_names = vec![];
    let mut ko_articles = vec![];

    iterate_children(&categories.data, &mut category_names, &mut ko_articles).await?;

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

    let mut file = File::create(
        Path::new("crawled_data")
            .join("category")
            .join("categories.json"),
    )?;
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

    let mut file = File::create(
        Path::new("crawled_data")
            .join("articles")
            .join(format!("{}.json", id)),
    )?;
    file.write(body.as_bytes())?;

    let article_response: ArticleResponse = serde_json::from_str(&body)?;

    Ok(article_response)
}