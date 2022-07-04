use std::env;
use indicatif::{ProgressBar, ProgressStyle};
use cli_table::{format::Justify, print_stdout, Cell, Style, Table};
use std::time::Duration;
use serde_json::json;
use spinners::{Spinner, Spinners};

//  Structs
#[derive(Clone, Debug)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String
}

#[derive(Clone, Debug)]
pub struct Domain {
    pub id: String,
    pub name: String,
    pub zone_id: String,
    pub locked: bool,
    pub dns_type: String,
    pub proxied: bool,
    pub proxiable: bool,
    pub content: String,
}


// API Variables
static API_URL: &str = "https://api.cloudflare.com/client/v4/zones";

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {

    let API_KEY: String =  env::args().nth(1).unwrap_or("".to_string());
    
    if(API_KEY == "") {
        println!("Please provide an API key as the first argument");
        std::process::exit(0);
    }

    
    // Public Variables
    let mut AvailableZones: Vec<Zone> = Vec::new();
    let mut AvailableDomains: Vec<Domain> = Vec::new();
    let mut ZonesToUpdate: Vec<Zone> = Vec::new();
    let mut DomainsToUpdate: Vec<Domain> = Vec::new();


    // Get CLI Arguments

    // Fetch Zones
    AvailableZones = fetch_zones(&API_KEY).await?;

    let mut available_zones_temp_vector = vec![];
    
    for (i, x) in AvailableZones.iter().enumerate() {
        available_zones_temp_vector.push(vec![
            i.clone().cell(),
            x.name.clone().cell(),
            x.status.clone().cell(),
        ]);
    }
    let available_zones_table = available_zones_temp_vector.table()
                                    .title(vec![
                                        "Num".cell().bold(true),
                                        "Name".cell().bold(true),
                                        "Status".cell().bold(true),
                                    ]).bold(true);

    println!("{}", available_zones_table.display().unwrap());

    println!("please select zones to update. (enter numbers separated by commas)");

    let mut zone_numbers_to_update = String::new();

    std::io::stdin().read_line(&mut zone_numbers_to_update).unwrap();

    let zone_numbers_to_update: Vec<i64> = zone_numbers_to_update.split(',').map(|x| x.trim().parse::<i64>().unwrap()).collect();

    for x in zone_numbers_to_update {
        ZonesToUpdate.push(AvailableZones[x as usize].clone());
    }

    // Fetch Domains
    AvailableDomains = fetch_domains(ZonesToUpdate, &API_KEY).await?;

    let mut available_domains_temp_vector = vec![];

    for(i, x) in AvailableDomains.iter().enumerate() {
        available_domains_temp_vector.push(vec![
            i.clone().cell(),
            x.name.clone().cell(),
            x.dns_type.clone().cell(),
        ]);
    }

    let available_domains_table = available_domains_temp_vector.table()
                                    .title(vec![
                                        "Num".cell().bold(true),
                                        "Name".cell().bold(true),
                                        "Type".cell().bold(true),
                                    ]).bold(true);

    println!("{}", available_domains_table.display().unwrap());

    println!("please select domains to update. (enter numbers separated by commas)");

    let mut domain_numbers_to_update = String::new();
    
    std::io::stdin().read_line(&mut domain_numbers_to_update).unwrap();

    let domain_numbers_to_update: Vec<i64> = domain_numbers_to_update.split(',').map(|x| x.trim().parse::<i64>().unwrap()).collect();

    for x in domain_numbers_to_update {
        DomainsToUpdate.push(AvailableDomains[x as usize].clone());
    }

    let mut current_ip = String::new();

    loop {

        let temp_current_ip_req = fetch_current_ip().await;

        let temp_current_ip = temp_current_ip_req.unwrap_or("".to_string());
        
        
        if (temp_current_ip != "".to_string()) && (temp_current_ip != current_ip) {
            current_ip = temp_current_ip;
            println!("IP Changed to: {}, Updating all the dns records", current_ip);
            for x in DomainsToUpdate.iter() {
                update_dns_records(vec!(x.clone()), current_ip.clone().as_str(), &API_KEY).await?;
            }
            println!("DNS Records Updated");
            current_ip = fetch_current_ip().await?;
        }
        let mut sp = Spinner::new(Spinners::Dots9, "Waiting for IP change...".into());
        std::thread::sleep(Duration::from_millis(1000));
        sp.stop();
    }

}


async fn fetch_zones(API_KEY: &str) -> Result<Vec<Zone>, reqwest::Error> {
    let client = reqwest::Client::new();

    let zone_fetching_progress_bar = ProgressBar::new(3);
    
    zone_fetching_progress_bar.set_style(ProgressStyle::default_bar()
    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
    .progress_chars("##-"));

    zone_fetching_progress_bar.set_message("Fetching Zones...");
    zone_fetching_progress_bar.inc(1);

    let  response: serde_json::Value = client.get(API_URL)
        .header("Authorization", format!("Bearer {}", API_KEY))
        .header("Content-Type", "application/json")
        .send()
        .await?
        .json()
        .await?;

    zone_fetching_progress_bar.inc(1);
    zone_fetching_progress_bar.set_message("Parsing Zones...");
    let temp = response["result"].as_array().unwrap();
    let zones: Vec<Zone> = temp.iter().map(|x| Zone {
        id: x["id"].as_str().unwrap().to_string(),
        name: x["name"].as_str().unwrap().to_string(),
        status: x["status"].as_str().unwrap().to_string()
    }).collect();
    
    zone_fetching_progress_bar.inc(1);
    zone_fetching_progress_bar.finish_with_message("Zones Fetched!");

    Ok(zones)
}

async fn fetch_domains(available_zones: Vec<Zone>, API_KEY: &str) -> Result<Vec<Domain>, reqwest::Error> {
    let client = reqwest::Client::new();
    let mut domains: Vec<Domain> = Vec::new();

    let zone_fetching_progress_bar = ProgressBar::new((available_zones.len() as u64)*3);

    zone_fetching_progress_bar.set_style(ProgressStyle::default_bar()
    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
    .progress_chars("##-"));


    for zone in available_zones {

        zone_fetching_progress_bar.inc(1);
        zone_fetching_progress_bar.set_message(format!("Fetching Domains for Zone {}...", zone.name));
        
        let  response: serde_json::Value = client.get(format!("{}/{}/dns_records", API_URL, zone.id).as_str())
            .header("Authorization", format!("Bearer {}", API_KEY))
            .header("Content-Type", "application/json")
            .send()
            .await?
            .json()
            .await?;

        let temp = response["result"].as_array().unwrap();

        zone_fetching_progress_bar.inc(1);
        zone_fetching_progress_bar.set_message(format!("Parsing Domains For Zone {}...", zone.name));

        let mut domains_in_zone: Vec<Domain> = temp.iter().map(|x| Domain {
            id: x["id"].as_str().unwrap().to_string(),
            name: x["name"].as_str().unwrap().to_string(),
            zone_id: x["zone_id"].as_str().unwrap().to_string(),
            locked: x["locked"].as_bool().unwrap(),
            dns_type: x["type"].as_str().unwrap().to_string(),
            proxied: x["proxied"].as_bool().unwrap(),
            proxiable: x["proxiable"].as_bool().unwrap(),
            content: x["content"].as_str().unwrap().to_string(),
        }).collect();
        for domain in domains_in_zone {
            if(domain.dns_type == "A") {
                domains.push(domain);
            }
        }
        zone_fetching_progress_bar.inc(1);
        zone_fetching_progress_bar.set_message(format!("Done Processing Domains From Zone {}...", zone.name));
    }

    zone_fetching_progress_bar.finish_with_message("Domains Fetched!");

    Ok(domains)
}


async fn fetch_current_ip() -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();

    let response: serde_json::Value = client.get("https://api.ipify.org?format=json")
        .send()
        .await?
        .json()
        .await?;


    Ok(response["ip"].as_str().unwrap_or("").to_string())
}


async fn update_dns_records(domains_to_update: Vec<Domain>, new_ip: &str, API_KEY: &str) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();

    let update_progress_bar = ProgressBar::new(domains_to_update.len() as u64);
    update_progress_bar.set_style(ProgressStyle::default_bar()
    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
    .progress_chars("##-"));
    update_progress_bar.set_message("Updating DNS Records...");
    let req_body = json!({
        "content": new_ip,
    });
    for domain in domains_to_update {
        let _response: serde_json::Value = client.patch(format!("{}/{}/dns_records/{}", API_URL, domain.zone_id, domain.id).as_str())
            .header("Authorization", format!("Bearer {}", API_KEY))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await?
            .json()
            .await?;
        update_progress_bar.inc(1);
        update_progress_bar.set_message(format!("Updated DNS Record for {}...", domain.name));
    }
    update_progress_bar.finish_with_message("DNS Records Updated!");
    Ok(())
}


async fn check_if_internet_connected() -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let response: serde_json::Value = client.get("https://api.ipify.org?format=json")
        .send()
        .await?
        .json()
        .await?;
    Ok(())
}
