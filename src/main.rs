extern crate hyper;
extern crate hyper_native_tls;
extern crate rustc_serialize;

use hyper::Client;
use std::env;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;

use rustc_serialize::Encodable;
use rustc_serialize::json::{self, Encoder, Json};


use std::io::Read;


fn get_current_event(organizer: String, token: String) -> Result<String, String> {
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);

    let search_url = format!("https://www.eventbriteapi.com/v3/events/search/?sort_by=date&organizer.id={organizer}&token={token}", organizer=organizer, token=token);
    let mut resp = client.get(&search_url).send().unwrap();
    let mut body = String::new();
    resp.read_to_string(&mut body).unwrap();

    let decoded = json::decode(&body).unwrap();

    Ok(decoded)
}

fn main() {
    let organizer = env::var("ORGANIZER_TOKEN").unwrap();
    let token = env::var("EVENTBRITE_TOKEN").unwrap();

    match get_current_event(organizer, token) {
        Ok(event) => println!("event : {:?}", event),
        Err(error) => println!("No event defined, error : {}", error)
    }
}