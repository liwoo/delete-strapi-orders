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
    //TODO: Capture the orderId which will be an optional
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root {
    pub data: Vec<DataElement>,
    pub meta: Meta,
}

#[tokio::main]
async fn main() {
    //TODO: Convert to CLI for delete customers and delete orders choice...
    let res = fetch_root_for_page(1).await;
    //TODO: Replace this with fetch meta
    match res {
        Ok(root) => process_root_orders(root).await, //need to give it meta
        Err(e) => println!("Error: {}", e),
    }
}

fn create_order_filter(page: i32, page_size: i32) -> String {
    format!("fields[0]=id&pagination[pageSize]={}&pagination[page]={}&publicationState=preview&locale[0]=en", page_size, page)
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
    //TODO: Get these from env variables
    const STRAPI_ORDERS_URL: &str = "https://localhost:1337/api/orders";
    const STRAPI_TOKEN: &str = "REPLACE_ME";
    let order_filters = create_order_filter(page, 10);
    let client = reqwest::Client::new();
    let header = format!("Bearer {}", STRAPI_TOKEN);
    let url = format!("{}?{}", STRAPI_ORDERS_URL, order_filters);
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
    let mut processed: i32 = 0;
    for _ in &root.data {
        //sleep for 1s
        print!(".");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        //replace this time with two calls to:
        //1. Delete the order in Shopify is it exists
        //2. Delete the order in Strapi
        processed += 1;
    }
    (processed, page)
}
