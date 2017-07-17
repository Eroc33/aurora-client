#![feature(conservative_impl_trait,range_contains)]
extern crate aurora_rs;
extern crate futures;
extern crate tokio_core;
extern crate tokio_service;
extern crate tokio_proto;
extern crate tokio_timer;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;
extern crate hyper;
extern crate mime;
extern crate chrono;
extern crate sun_times;
#[macro_use]
extern crate error_chain;

use aurora_rs as aurora;

use std::time::Duration;
use std::net::SocketAddr;
use std::io::Write;

use futures::{Future,Stream};
use tokio_core::reactor::{Core,Handle};
use tokio_service::Service;
use tokio_proto::TcpClient;
use tokio_timer::{Timer,TimerError};

use chrono::{Local,Utc,TimeZone};

use hyper::Method;
use hyper::StatusCode;
use hyper::client::Request as HttpRequest;

use aurora::{AuroraProto,Request,Response,CumulativeDuration,MeasurementType};

mod errors;
mod ratelimitedstream;

use ratelimitedstream::StreamExt;

#[derive(Debug,Clone,Deserialize)]
struct PvOutputConfig{
    ///Pvoutput.org sid
    system_id: String,
    ///Pvoutput.org api key
    api_key: String,
}

#[derive(Debug,Clone,Deserialize)]
struct LocationConfig{
    latitude: f64,
    longitude: f64,
    elevation: f64,
}

#[derive(Debug,Clone,Deserialize)]
struct SerialConfig{
    ///The address on which the client will connect to the tcp->serial bridge
    tcp_address: SocketAddr,
    ///The aurora protocol address
    aurora_address: u8,
    ///The time between requests to the inverter
    poll_duration: Duration,
    ///The number of times to wait `poll_duration` before failing
    timeout_mul: u32,
}

#[derive(Debug,Clone,Deserialize)]
struct Config{
    ///PVOutput.org config
    serial: SerialConfig,
    pv_output: PvOutputConfig,
    location: LocationConfig,
}

//creates a custom timer with a longer tick_duration to allow longer (but marginally less accurate) timeouts
fn timer() -> Timer{
    tokio_timer::wheel().tick_duration(Duration::from_secs(1)).build()
}


///Creates a stream of (cumulative energy, current voltage) values
///from an aurora protocol client connected to the serial port, and the inverter address
fn energy_voltage_stream<S>(client:S,addr:u8) -> impl Stream<Item=(u32,f32),Error=S::Error>
where S: Service<Request=(u8,Request),Response=Response>,
      S::Error: std::fmt::Debug + From<TimerError>
{
    futures::stream::unfold(client,move |client|{
        Some(
            client.call((addr,Request::CumulativeEnergy(CumulativeDuration::Daily)))
            .map(move |energy| (energy,client))
            .and_then(move |(energy,client)|{
                client.call((addr,Request::Measure{type_:MeasurementType::Input1Voltage,global:true}))
                    .map(move |voltage| ((energy,voltage),client))
            })
        )
    })
    .filter_map(|res|{
        match res{
            (Response::CumulativeEnergy{value,..},Response::Measure{val,..}) => Some((value,val)),
            _ => None
        }
    })
}

fn load_config() -> std::io::Result<Config>{
    use std::io::Read;

    let mut cfg_file = std::fs::File::open("Config.toml")?;
    let mut file_contents = String::new();
    cfg_file.read_to_string(&mut file_contents)?;
    let cfg = toml::from_str(&file_contents).expect("Invalid toml");
    Ok(cfg)
}

fn upload_request<C: hyper::client::Connect>(client: hyper::Client<C>, request: HttpRequest) -> impl Future<Item=hyper::Client<C>,Error=errors::Error>
{
    client.request(request)
        .map_err(errors::Error::from)
        .and_then(move |res|{
            //A single failure is not a stream failure, but we should still warn the
            //user
            if res.status() != StatusCode::Ok{
                eprintln!("[WARNING]: Failed to upload status, continuing");
            }
            Ok(client)
        })

}

fn mainloop(core: &mut Core, handle: &Handle, serial_cfg: &SerialConfig, pvoutput_cfg: &PvOutputConfig) -> Result<(),errors::Error>
{
    let poll_duration = serial_cfg.poll_duration;
    let timeout_duration = poll_duration*serial_cfg.timeout_mul;
    let timer = timer();

    let client = TcpClient::new(AuroraProto)
        .connect(&serial_cfg.tcp_address,&core.handle())
        .map_err(errors::Error::from)
        .and_then(|client|{
            println!("Connected");
            
            let ev_stream = energy_voltage_stream(client,serial_cfg.aurora_address);
            let ev_stream = ev_stream.rate_limited(poll_duration,timer.clone(),2);

            timer.timeout_stream(ev_stream,timeout_duration)
                .map_err(errors::Error::from)
                //Convert values to requests
                .map(move |(cum_e,cur_v)|{
                    println!("{}Wh, {}V",cum_e,cur_v);
                    let mut req = HttpRequest::new(Method::Post,"http://pvoutput.org/service/r2/addstatus.jsp".parse().expect("Hardcoded url is invalid?"));
                    {
                        use hyper::header::*;

                        let headers = req.headers_mut();
                        headers.set_raw("X-Pvoutput-Apikey",pvoutput_cfg.api_key.clone());
                        headers.set_raw("X-Pvoutput-SystemId",pvoutput_cfg.system_id.clone());
                        headers.set(ContentType(mime::APPLICATION_WWW_FORM_URLENCODED));
                    }
                    let now = Local::now();
                    let date = now.format("%Y%m%d");
                    let time = now.format("%H:%M");
                    let body = format!("d={}&t={}&v1={}&v6={}",date,time,cum_e,cur_v);
                    //println!("Body: {}",body);
                    req.set_body(body);
                    req
                })
            //upload stream
            .fold(hyper::Client::new(handle),upload_request)
            .map(|_| ())
        });
    core.run(client)
}

pub fn run_service() -> errors::Result<()>{
    let cfg = load_config().expect("Couldn't load config");
    let Config{serial,location,pv_output} = cfg;
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    loop{
        let (sunrise,sunset) = sun_times::sun_times(Utc::today(),location.latitude,location.longitude,location.elevation);
        let (sunrise,sunset) = (Local.from_utc_datetime(&sunrise.naive_utc()).time(),Local.from_utc_datetime(&sunset.naive_utc()).time());
        let day_start = chrono::NaiveTime::from_hms(0,0,0);
        let day_end = chrono::NaiveTime::from_hms(23,59,59);

        let now = Local::now().time();

        let daytime = (sunrise..sunset).contains(now);
        println!("Is daytime? {}",daytime);

        if daytime {
            //sunlight hours
            let result = mainloop(&mut core, &handle, &serial, &pv_output);
            match result {
                Ok(_) => (),
                //This probably means the tcp-serial bridge timed out so just continue
                Err(errors::Error(errors::ErrorKind::Io(ref ioe),_)) if ioe.kind() == std::io::ErrorKind::BrokenPipe  => (),
                Err(e) => bail!(e),
            }
        }else{
            let time_to_sunrise = (day_end.signed_duration_since(now)) + (sunrise.signed_duration_since(day_start));
            std::thread::sleep(time_to_sunrise.to_std().expect("Duration out of range"))
        }

    }
    Ok(())
}