use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use headless_chrome::{Browser, Element, LaunchOptions, Tab};

fn get_year() -> String {
    let now: DateTime<Utc> = Utc::now();
    let now: DateTime<FixedOffset> =
        now.with_timezone(&FixedOffset::east_opt(9 * 3600).expect("Could not get timezone"));
    let year = now.format("%Y").to_string();
    year
}

fn get_period(tab: &Arc<Tab>) -> String {
    let year = get_year();
    let period_element_selector = "h2.ly-mod-ttl-l";
    let period_element = tab.wait_for_element(period_element_selector);
    let period_text = period_element
        .map(|e| e.get_inner_text().expect("Could not get period text"))
        .expect("Could not find period element");
    let period = period_text.split_ascii_whitespace().collect::<Vec<&str>>()[0].to_string();
    let re = Regex::new(r"\d+/\d+").expect("Could not create regex");
    let dates = period
        .split("～")
        .map(|day| {
            let day = re.find(day).expect("Could not find date").as_str();
            let date = format!("{}/{}", year, day);
            NaiveDate::parse_from_str(date.as_str(), "%Y/%m/%d")
                .expect("Could not parse date")
                .format("%Y%m%d")
                .to_string()
        })
        .collect::<Vec<_>>();
    dates.join("-")
}

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,
    description: String,
    src: String,
}

fn get_persons(tab: &Arc<Tab>) -> Vec<Person> {
    let persons_list_element_selector = ".ly-mod-layout-3clm .ly-mod-layout-clm";
    let persons_list_element = tab
        .wait_for_elements(persons_list_element_selector)
        .expect("Could not find persons list element");
    let persons = persons_list_element.iter().map(|e| {
        let src_element = e.find_element("img").expect("Could not find src element");
        let attributes = src_element.get_attributes().expect("Could not get src");
        let attributes = match attributes {
            Some(s) => s,
            None => panic!("Could not get src"),
        };
        let index = attributes
            .iter()
            .position(|s| s == "src")
            .expect("Could not get src");
        let prefix = "https://www.family.co.jp".to_string();
        let src = attributes[index + 1].to_string();
        let src = prefix + &src;

        let name_element = e.find_element("b").expect("Could not find name element");
        let name = name_element
            .get_inner_text()
            .expect("Could not get name text");

        let description_element = e
            .find_element(".ly-txt p")
            .expect("Could not find description element");
        let description = description_element
            .get_inner_text()
            .expect("Could not get description text");

        Person {
            name,
            description,
            src,
        }
    });
    return persons.collect::<Vec<_>>();
}

#[derive(Debug, Serialize, Deserialize)]
struct Music {
    title: String,
    artist: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MusicTable {
    time: String,
    musics: Vec<Music>,
}

fn get_musics(tab: &Arc<Tab>) -> Vec<MusicTable> {
    let music_tables_element_selector = "#contents > div > div.par.parsys > div.free_html_element.parbase.section > div > div.ly-mainarea-slide.js-mainvisual-list.slick-initialized.slick-slider > div > div > div > div:nth-child(6) > section > div > div:nth-child(3) > div > div > div";
    let music_tables_elements = tab
        .wait_for_elements(music_tables_element_selector)
        .expect("Could not find music tables element");

    let all_time_table = &music_tables_elements[1]
        .find_element("table")
        .expect("Could not find all time table");
    let am_tables = &music_tables_elements[2]
        .find_elements("table")
        .expect("Could not find am tables");
    let pm_tables = &music_tables_elements[3]
        .find_elements("table")
        .expect("Could not find pm tables");

    let morning_table = &am_tables[0];
    let daytime_table = &am_tables[1];
    let night_table = &pm_tables[0];
    let midnight_table = &pm_tables[1];

    struct TableElement<'a> {
        time: String,
        table: &'a Element<'a>,
    }

    let tables = vec![
        TableElement {
            time: "all_time".to_string(),
            table: all_time_table,
        },
        TableElement {
            time: "morning".to_string(),
            table: morning_table,
        },
        TableElement {
            time: "daytime".to_string(),
            table: daytime_table,
        },
        TableElement {
            time: "night".to_string(),
            table: night_table,
        },
        TableElement {
            time: "midnight".to_string(),
            table: midnight_table,
        },
    ];

    let musics_each_time = tables
        .iter()
        .map(|table| {
            let rows = table
                .table
                .find_elements("tbody > tr")
                .expect("Could not find rows in table");
            let musics = rows
                .iter()
                .map(|row| {
                    let cells = row
                        .find_elements("td")
                        .expect("Could not find cells in row");
                    let title = cells[0].get_inner_text().expect("Could not get title");
                    let artist = cells[1].get_inner_text().expect("Could not get artist");
                    Music { title, artist }
                })
                .collect::<Vec<_>>();
            MusicTable {
                time: table.time.clone(),
                musics,
            }
        })
        .collect::<Vec<_>>();
    musics_each_time
}

#[tokio::main]
async fn main() -> reqwest::Result<()> {
    let launch_options = LaunchOptions::default_builder()
        .headless(true)
        .build()
        .unwrap();
    let browser = Browser::new(launch_options).expect("Failed to launch headless browser");

    let tab = browser.new_tab().expect("Failed to open new tab");

    tab.navigate_to("https://www.family.co.jp/campaign/radio.html")
        .expect("Failed to navigate");

    let period = get_period(&tab);

    let persons = get_persons(&tab);

    let musics_each_time = get_musics(&tab);

    #[derive(Debug, Serialize, Deserialize)]
    struct Mixfam {
        period: String,
        persons: Vec<Person>,
        musics: Vec<MusicTable>,
    }

    let mixfam = Mixfam {
        period,
        persons,
        musics: musics_each_time,
    };

    let json = serde_json::to_string(&mixfam).unwrap();

    println!("JSON: {}", json);

    let client = reqwest::Client::new();
    let res = client.post("https://script.google.com/macros/s/AKfycbzGpqxDa-8nTcS-SWGiTCY_02g6Mq0sKeNfOsFQS3tm1DGZ0BRRYwNI4ci8fDsQ3t64/exec")
        .json(&mixfam)
        .send()
        .await?
        .text()
        .await?;

    println!("Response: {}", res);

    Ok(())
}
