extern crate hyper;
extern crate hyper_native_tls;
extern crate rustc_serialize;
extern crate rand;

use std::error::Error;
use std::fmt::{self, Display};
use hyper::Client;
use hyper::header::Connection;
use std::env;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;
use rustc_serialize::json;
use std::io::Read;
use std::ops::Range;
use rand::{thread_rng, sample};

#[derive(Debug)]
enum LotteryError {
    NoEventAvailable
}

impl Error for LotteryError {
    fn description(&self) -> &str {
        match *self {
            LotteryError::NoEventAvailable => "No event available",
        }
    }
}

impl Display for LotteryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LotteryError -> No event available")
    }
}


#[derive(RustcDecodable, Debug)]
struct Pagination {
    object_count: u8,
    page_count: u8,
    page_size: u8,
    page_number: u8
}

#[derive(RustcDecodable, Debug, Clone)]
struct Event {
    id: String
}

#[derive(RustcDecodable, Debug)]
struct Events {
    events: Vec<Event>,
    pagination: Pagination
}

#[derive(RustcDecodable, RustcEncodable, Debug, Clone)]
struct Profile {
    first_name: String,
    last_name: String
}

#[derive(RustcDecodable, Debug, Clone)]
struct Attende {
    profile: Profile
}

#[derive(RustcDecodable, Debug)]
struct Attendees {
    attendees: Vec<Attende>,
    pagination: Pagination
}

fn https_client() -> hyper::Client {
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    Client::with_connector(connector)
}

fn fetch(url: &str) -> Result<hyper::client::Response, Box<Error>> {
    https_client().get(url)
            .header(Connection::close())
            .send()
            .map_err(|err| From::from(err))
}

fn json<T: rustc_serialize::Decodable>(mut resp: hyper::client::Response) -> Result<T, Box<Error>> {
    let mut body = String::new();
    resp.read_to_string(&mut body).unwrap();
    json::decode(&body)
        .map_err(|err| From::from(err))
}

fn get_current_event (organizer: &str, token: &str) -> Result<Event, Box<Error>> {
    fetch(&format!("https://www.eventbriteapi.com/v3/events/search/?sort_by=date&organizer.id={organizer}&token={token}", organizer=organizer, token=token))
        .and_then(json)
        .and_then(|result: Events| result.events.first().map(|reference| reference.clone()).ok_or(Box::from(LotteryError::NoEventAvailable)))
}

fn attendees_url(event_id: &str, token: &str, page: u8) -> String {
    format!("https://www.eventbriteapi.com/v3/events/{event_id}/attendees/?token={token}&page={page}", event_id=event_id, token= token, page=page)
}

fn concat<T: Clone>(first: &Vec<T>, second: &Vec<T>) -> Vec<T> {
    let mut result = first.to_vec();
    result.extend(second.iter().cloned());
    result
}

fn get_attendees(event_id: &str, token: &str) -> Result<Vec<Profile>, Box<Error>> {
    fetch(&attendees_url(event_id, token, 1))
        .and_then(json)
        .map(|result: Attendees| {
            let range = Range{start: result.pagination.page_number, end: result.pagination.page_count};
            range.fold(result.attendees, |first, page| {
                    println!("Fetch for page {}", page+1);
                    let fetched:Attendees = fetch(&attendees_url(event_id, token, page+1)).and_then(json).unwrap();
                    concat(&first, &fetched.attendees)
                })
            })
        .map(|attendees: Vec<Attende>| attendees.into_iter().map(|attendee| attendee.profile).collect())
}


fn main() {
    let mut rng = thread_rng();
    let organizer = env::var("ORGANIZER_TOKEN").unwrap();
    let token = env::var("EVENTBRITE_TOKEN").unwrap();

    match get_current_event(organizer.as_str(), token.as_str()) {
        Ok(event) => println!("event : {:?}", event),
        Err(error) => println!("No event defined, error : {}", error)
    }
    match get_attendees("34166417675", token.as_str()) {
        Ok(attendes) => println!("{:?}", sample(&mut rng, attendes, 3)),
        Err(error) => println!("No attendees, error : {}", error)
    }
}