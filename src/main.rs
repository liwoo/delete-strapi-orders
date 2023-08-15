use clap::Parser;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub page: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
    #[serde(rename = "pageCount")]
    pub page_count: i32,
    pub total: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub pagination: Pagination,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataElement {
    pub id: i32,
    pub attributes: DataAtribute,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataAtribute {
    #[serde(rename = "cartReference")]
    pub cart_reference: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root {
    pub data: Vec<DataElement>,
    pub meta: Meta,
}

#[derive(Debug)]
struct ShopifyConfig {
    access_token: String,
    shop_url: String,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long)]
    delete: String,
}

#[derive(Debug)]
struct StrapiConfig {
    base_url: String,
    auth_token: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let cli = Cli::parse();

    match cli.delete.as_str() {
        "orders" => delete_orders().await,
        "customers" => delete_customers().await,
        _ => println!("Invalid command"),
    }
}

fn load_configs() -> (ShopifyConfig, StrapiConfig) {
    let strapi_url = std::env::var("STRAPI_BASE_URL").expect("STRAPI_BASE_URL not set");
    let strapi_token = std::env::var("STRAPI_TOKEN").expect("STRAPI_TOKEN not set");
    let shopify_token = std::env::var("SHOP_ACCESS_TOKEN").expect("SHOP_ACCESS_TOKEN not set");
    let shopify_url = std::env::var("SHOP_BASE_URL").expect("SHOP_BASE_URL not set");

    (
        ShopifyConfig {
            access_token: shopify_token,
            shop_url: shopify_url,
        },
        StrapiConfig {
            base_url: strapi_url,
            auth_token: strapi_token,
        },
    )
}

async fn delete_orders() {
    let res = fetch_root_for_page(1).await;
    //TODO: Replace this with fetch meta
    match res {
        Ok(root) => process_root_orders(root).await, //need to give it meta
        Err(e) => println!("Error: {}", e),
    }
}

async fn delete_customers() {
    load_configs();
    println!("Deleting customers");
}

fn create_order_filter(page: i32, page_size: i32) -> String {
    format!("fields[0]=id&fields[1]=cartReference&pagination[pageSize]={}&pagination[page]={}&publicationState=preview&locale[0]=en", page_size, page)
}

async fn process_root_orders(root: Root) {
    //1. Get total pages
    let total_pages = root.meta.pagination.page_count;
    let total_orders = root.meta.pagination.total;
    let mut processed_orders = 0;
    println!("About to start deleting: {}", total_orders);

    //2. Loop through pages
    let tasks: Vec<_> = (1..=total_pages)
        .into_iter()
        .map(|page| {
            tokio::spawn(async move {
                let new_root = fetch_root_for_page(page).await;
                match new_root {
                    Ok(next) => process_paged_orders(&next, page).await,
                    Err(e) => {
                        println!("Error: {}", e);
                        (0, page)
                    }
                }
            })
        })
        .collect();

    let results = futures::future::join_all(tasks).await;

    for result in results {
        match result {
            Ok(result_values) => {
                let (total_processed_orders, result_page) = result_values;
                processed_orders += total_processed_orders;
                println!(
                    "Processed {} of {} 🧾 Orders (Page -> {} of {})",
                    processed_orders, total_orders, result_page, total_pages
                );
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}

async fn fetch_root_for_page(page: i32) -> Result<Root, reqwest::Error> {
    let strapi_orders_url: String = format!(
        "{}/orders",
        std::env::var("STRAPI_BASE_URL").unwrap().as_str()
    );
    let strapi_token: String = std::env::var("STRAPI_TOKEN").unwrap();

    let order_filters = create_order_filter(page, 10);
    let client = reqwest::Client::new();
    let header = format!("Bearer {}", strapi_token);
    let url = format!("{}?{}", strapi_orders_url, order_filters);
    //add headers
    let res = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Authorization", &header)
        .send()
        .await?
        .json::<Root>()
        .await;

    return res;
}

async fn process_paged_orders(root: &Root, page: i32) -> (i32, i32) {
    //process and handle exceptions per order
    let (shopify_config, strapi_config) = load_configs();
    let mut processed: i32 = 0;
    for data in &root.data {
        print!(".");
        if data.attributes.cart_reference.is_some() {
            delete_shopify_resource(
                &shopify_config,
                "orders",
                data.attributes.cart_reference.as_ref().unwrap().to_string(),
            )
            .await;
        }
        delete_strapi_resource(&strapi_config, "order", data.id).await;
        processed += 1;
    }
    (processed, page)
}

async fn delete_shopify_resource(config: &ShopifyConfig, resource: &str, res_id: String) -> bool {
    let url = format!("{}/{}/{}.json", config.shop_url, resource, res_id);

    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert(
        "X-Shopify-Access-Token",
        config.access_token.parse().unwrap(),
    );

    let response = client.delete(&url).headers(headers).send().await;

    match response {
        Ok(_) => true,
        Err(_) => false,
    }
}

async fn delete_strapi_resource(config: &StrapiConfig, resource: &str, res_id: i32) -> bool {
    let url = format!("{}/{}/{}", config.base_url, resource, res_id);

    let client = reqwest::Client::new();
    let response = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", config.auth_token))
        .send()
        .await;

    match response {
        Ok(_) => true,
        Err(_) => false,
    }
}
