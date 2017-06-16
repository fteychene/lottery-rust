#![feature(drop_types_in_const)]
#![feature(box_patterns)]
extern crate hyper;
extern crate hyper_native_tls;
extern crate rustc_serialize;
extern crate rand;
extern crate iron;
extern crate router;
extern crate urlencoded;
#[macro_use] extern crate log;
extern crate env_logger;

use std::error::Error;
use std::fmt::{self, Display};
use hyper::Client;
use std::env;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;
use rustc_serialize::json;
use std::io::Read;
use std::ops::Range;
use rand::{thread_rng, sample};

use iron::prelude::*;
use iron::status;
use hyper::header::{Connection, ContentType};
#[allow(unused_imports)] use hyper::mime::*; // Import macro mine!
use iron::modifiers::Header;
use router::Router;
use urlencoded::UrlEncodedQuery;

use std::{thread, time};
use std::mem;

#[derive(Debug)]
enum LotteryError {
    NoEventAvailable,
    TechnicalError(Box<Error>),
    MissingArgument(String),
    InvalidArgument(String, String),
}

impl Error for LotteryError {
    fn description<'a>(&'a self) -> &'a str {
        match *self {
            LotteryError::NoEventAvailable => "No event available",
            LotteryError::TechnicalError(_) => "Technical error",
            LotteryError::MissingArgument(_) => "Missing argument",
            LotteryError::InvalidArgument(_, _) => "Invalid argument"
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            LotteryError::NoEventAvailable => None,
            LotteryError::TechnicalError(ref error) => Some(error.as_ref()),
            LotteryError::MissingArgument(_) => None,
            LotteryError::InvalidArgument(_, _) => None
        }
    }
}

impl Display for LotteryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LotteryError::NoEventAvailable => write!(f, "No event available"),
            LotteryError::TechnicalError(ref error) => write!(f, "Technical error: {}", error.as_ref()),
            LotteryError::MissingArgument(ref arg) => write!(f, "Missing argument {}", arg),
            LotteryError::InvalidArgument(ref arg, ref reason) => write!(f, "Invalid argument {} reason : {}", arg, reason)
        }
    }
}

impl From<LotteryError> for Response {
    fn from(err: LotteryError) -> Response {
        let status = match err {
            LotteryError::NoEventAvailable => status::Gone,
            LotteryError::TechnicalError(_) => status::InternalServerError,
            LotteryError::MissingArgument(_) => status::BadRequest,
            LotteryError::InvalidArgument(_, _) => status::BadRequest
        };
        json_response(status, format!("{}", err))
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

fn fetch(url: &str) -> Result<hyper::client::Response, LotteryError> {
    https_client().get(url)
            .header(Connection::close())
            .send()
            .map_err(|err| LotteryError::TechnicalError(Box::from(err)))
}

fn json<T: rustc_serialize::Decodable>(mut resp: hyper::client::Response) -> Result<T, LotteryError> {
    let mut body = String::new();
    resp.read_to_string(&mut body).unwrap();
    json::decode(&body)
        .map_err(|err| LotteryError::TechnicalError(Box::from(err)))
}

fn get_current_event (organizer: &str, token: &str) -> Result<Event, LotteryError> {
    fetch(&format!("https://www.eventbriteapi.com/v3/events/search/?sort_by=date&organizer.id={organizer}&token={token}", organizer=organizer, token=token))
        .and_then(json)
        .and_then(|result: Events| result.events.first().map(|reference| reference.clone()).ok_or(LotteryError::NoEventAvailable))
}

fn attendees_url(event_id: &str, token: &str, page: u8) -> String {
    format!("https://www.eventbriteapi.com/v3/events/{event_id}/attendees/?token={token}&page={page}", event_id=event_id, token= token, page=page)
}

fn concat<T: Clone>(first: &Vec<T>, second: &Vec<T>) -> Vec<T> {
    let mut result = first.to_vec();
    result.extend(second.iter().cloned());
    result
}

fn get_attendees(event_id: &str, token: &str) -> Result<Vec<Profile>, LotteryError> {
    fetch(&attendees_url(event_id, token, 1))
        .and_then(json)
        .map(|result: Attendees| {
            let range = Range{start: result.pagination.page_number, end: result.pagination.page_count};
            range.fold(result.attendees, |first, page| {
                    let fetched:Attendees = fetch(&attendees_url(event_id, token, page+1)).and_then(json).unwrap();
                    concat(&first, &fetched.attendees)
                })
            })
        .map(|attendees: Vec<Attende>| attendees.into_iter().map(|attendee| attendee.profile).collect())
        .map_err(|err| LotteryError::TechnicalError(Box::from(err)))
}

fn json_response<T: rustc_serialize::Encodable> (status: iron::status::Status, body: T) -> Response {
    Response::with((status, json::encode(&body).unwrap(), Header(ContentType(mime!(Application/Json; Charset=Utf8)))))
}

fn get_nb_winners(req: &mut Request) -> Result<u8, LotteryError> {
    req.get_ref::<UrlEncodedQuery>()
        .map_err(|err| LotteryError::TechnicalError(Box::from(err)))
        .and_then(|params| params.get("nb").and_then(|args| args.first()).ok_or(LotteryError::MissingArgument(String::from("nb"))))
        .and_then(|value| value.parse::<u8>().map_err(|_| LotteryError::InvalidArgument(String::from("nb"), String::from("Parameter nb should be a positive integer"))))
        .map_err(|err| From::from(err))
}

fn winners(req: &mut Request) -> IronResult<Response> {
    unsafe {
        let mut rng = thread_rng();
        match get_nb_winners(req)
                .and_then(|nb| CACHE.clone().map(|attendees| sample(&mut rng, attendees, nb as usize)).ok_or(LotteryError::NoEventAvailable)) {
            Ok(result) => Ok(json_response(status::Ok, result)),
            Err(error) => {
                error!("Error during request execution => {}", error);
                Ok(From::from(error))
            }
        }
    }
}

unsafe fn cache_loop(attendees: &mut Option<Vec<Profile>>, organizer: &str, token: &str, timeout: u64) {
    loop {
        println!("Fetch last event and attendees from eventbrite");
        
        match get_current_event(organizer, token).and_then(|event| get_attendees(&event.id, token)) {
            Ok(current_attendees) => {mem::replace(attendees, Some(current_attendees));},
            Err(_err) => {mem::replace(attendees, None);}
        }
        
        /*
        match get_attendees("34166417675", token) {
            Ok(current_attendees) => {mem::replace(attendees, Some(current_attendees));},
            Err(err) => println!("Error while fetching attendees {}", err)
        }
        */
        thread::sleep(time::Duration::from_secs(timeout));
    }
}

static mut CACHE: Option<Vec<Profile>> = None;

fn main() {
    env_logger::init().unwrap();
    let organizer = env::var("ORGANIZER_TOKEN").unwrap();
    let token = env::var("EVENTBRITE_TOKEN").unwrap();

    unsafe {
        thread::spawn(move || cache_loop(&mut CACHE, organizer.as_str(), token.as_str(), 3600));
    }

    let mut router = Router::new();
    router.get("/winners", winners, "query");

    Iron::new(router).http("localhost:3000").unwrap();
}