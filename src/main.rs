// Idea: CLI tool, web crawler. Provided an URL as a parameter, parses a page and follows all the links found, crawling deeper and deeper to the web (BFS pattern).
// Gathers all the pictures (png, jpeg?) found, but drops very small ones (small icons, lines, etc., perhaps < 1kb)
// Continues crawling until N pictures are gathered  (N is a param>0)
// For every image, program creates a thumbnail of height (e.g. 300px)
// In the end program creates a big collage (single png/jpg file), containing a table of all the thumbnails
// optional: rende am image index [0..N) on every thumbnail

// usage:
// crawl http://somesite.com 1000


//use std::io;
use url::{Url, ParseError};
use std::env;
use anyhow::{anyhow, Context, Result};
use std::io;
//use std::io::Read;
use std::fs;
use std::fs::File;
use std::iter::FromIterator;

use html_parser::{Dom, Node};

use std::collections::{HashSet, VecDeque};
use reqwest::blocking::Response;

//use chrono::DateTime;
use chrono::prelude::*;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    println!("Crawl!");

    match run() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("Crawling error: {}", e);
            std::process::exit(1);
        }
    }
}


fn run() -> Result<()> {
    let (url, _n) = parse_arg()?;

    // TODO: check URL
    
    // create folder
    let folder: String = Utc::now().to_rfc3339(); 
    fs::create_dir(&folder)?;

    let img_urls = crawl_bfs(&url, _n)?;

    download_images(img_urls, &folder)?;


    Ok(())
}


fn parse_arg() -> Result<(Url, u32), anyhow::Error> {

    let param1 = env::args().nth(1).context("Failed to find first parameter")?;

    // PARSE URL
    let url = Url::parse(&param1).map_err(|e| anyhow!(format!("Fail to parse: {}",e)))?;
    //let _url2 = Url::parse(&param1).context("failed to parse")?;
        
    // PARSE AMOUNT OF PICS
    let param2 = env::args().nth(2).context("Failed to find second parameter")?;
    let n = param2.parse::<u32>().with_context(|| format!("Failed to parse arg2. Not a number? {}", param2))?;
    println!("Amount of pictures needed: {}", n);
    
    Ok((url,n))
}


fn crawl_bfs(url: &Url, n: u32) -> Result<Vec<String>, anyhow::Error> 
{
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    let mut i = n;

    let mut all_image_urls: HashSet<String> = HashSet::new();

    queue.push_back(url.to_string());

    while !queue.is_empty() && i>0 {
        println!("Crawl page {}", n-i);
        let url: String = queue.pop_front().unwrap_or_default();

        let opt = Url::parse(&url);
        if opt == Err(ParseError::RelativeUrlWithoutBase) 
        {
            println!("shitty href");
            continue;
        }

        
        let parsed = opt.unwrap();
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            println!("shitty scheme ({})", parsed.scheme());
            continue;
        }

        println!("{}", url);

        if visited.contains(&url) {
            i-=1;
            continue;
        } else {
            visited.insert(url.clone());
        }

        let body = match reqwest::blocking::get(url) {
            Ok(res) => res.text()?,
            Err(e) => {
                println!("HTTP get [{}]. skip this site", e);
                "".to_string()
            }
        };

        if body.is_empty() { continue }

        let dom = match Dom::parse(&body) {
            Ok(val) => val,
            Err(_) => {
                println!("parse error. skip this site");
                Dom::parse("<div></div>")?
            }
        };

        let hrefs = collect_hrefs(&dom);
        for h in hrefs.iter() {
            if !visited.contains(h) {
                queue.push_back(h.to_string());
            }
        }

        // collect pictures
        // problem: no picture size known on this step, so we don't know whether to count it
        let image_urls = collect_img_sources(&dom);
        println!("collected {} images!", image_urls.len());
        for src in image_urls.iter() {
            if !all_image_urls.contains(src) {
                // TODO: optimize (not copy but change owner)
                //println!("    img: {}", src);
                all_image_urls.insert(src.clone());
            }
        }

        i-=1;
    }

    Ok(Vec::from_iter(all_image_urls))
}

fn collect_hrefs(dom: &Dom) -> Vec<String>
{
    let iter = dom.children.get(0).unwrap().into_iter();
    let mut hrefs = Vec::new();

    for node in iter {
        match node {
            Node::Element(ref elem) => {
                if elem.name == "a" {
                    //println!("{:?}", elem);
                    let href = if elem.attributes.contains_key("href") { elem.attributes["href"].clone().unwrap() } else { String::new() };
                    if Url::parse(&href) == Err(ParseError::RelativeUrlWithoutBase) { continue; }
                    
                    hrefs.push(href.to_string());
                }
            },
            _ => ()
        }
    }

    return hrefs;
}

fn collect_img_sources(dom: &Dom) -> Vec<String> {

    if dom.children.is_empty() {
        return vec![];
    }

    let iter = dom.children.get(0).unwrap().into_iter();
    let mut image_urls = Vec::new();

    for node in iter {
        match node {
            Node::Element(ref elem) => {
                if elem.name == "img" {
                    let img = if elem.attributes.contains_key("src") { elem.attributes["src"].clone().unwrap_or_default() } else { String::new() };
                    image_urls.push(img);
                }
            },
            _ => ()
        }
    }

    return image_urls;
}

fn download_images(image_urls: Vec<String>, folder: &String) -> Result<()> {

    for url in image_urls.iter() {
        download_image(url, folder)?;
    }

    Ok(())
}

fn download_image(src: &String, folder: &String) -> Result<()> {

    let name = src.split("/").last().unwrap();
    if name.is_empty() {
        return Ok(());
    }

    let mut path = String::new();
    path.push_str(&folder); path.push_str("/"); path.push_str(name);

    println!("image src {}", src);
    let mut result = reqwest::blocking::get(src);

    match result {
        Ok(_) => (),
        Err(e) => {
            println!("Image [{}]. skip this image", e);
            return Ok(());
        }
    };
    
    let mut resp = result.unwrap();
    let len = resp.content_length().unwrap();

    if len < 4096 { return Ok(()); }
    println!("file {} size {}", name, len);
    let mut out = File::create(path)?;
    io::copy(&mut resp, &mut out)?;

    Ok(())
}
