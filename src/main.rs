extern crate prometheus;

extern crate nvml_wrapper;

use nvml_wrapper::NVML;
use nvml_wrapper::enum_wrappers::device::{TemperatureSensor, Clock};

use hyper::{header::CONTENT_TYPE, rt::Future, service::service_fn_ok, Body, Response, Server};

use prometheus::{Opts, Encoder, Gauge, IntGauge, GaugeVec, IntGaugeVec, TextEncoder, Registry};

const NAMESPACE: &str = "nvidia_gpu";
const LABELS: [&'static str; 3] = ["minor_number", "uuid", "name"];

// TODO: https://lh3.googleusercontent.com/1GLnuV66rZqTmWQJ1QXW6f8yz1rCLJ9tIzq4RgsEA_qhBOq72KJCBgXeLdc0EXWePx9E-stlEZPShJXeh2WEOtVx-iAOv38cJiApQRn9iA0uqmTnc5vINK2me1vGBxmz-IiCarlN

// Error types

type Result<T> = std::result::Result<T, CollectingError>;

#[derive(Debug)]
enum CollectingError {
    Nvml(nvml_wrapper::error::Error),
    Prometheus(prometheus::Error),
}

impl From<nvml_wrapper::error::Error> for CollectingError {
    fn from(err: nvml_wrapper::error::Error) -> CollectingError {
        CollectingError::Nvml(err)
    }
}

impl From<prometheus::Error> for CollectingError {
    fn from(err: prometheus::Error) -> CollectingError {
        CollectingError::Prometheus(err)
    }
}

struct Collector {
    nvml: NVML,
    registry: Registry,
    num_devices_gauge: IntGauge,
    gpu_utilization_gauge: IntGaugeVec,
    memory_utilization_gauge: IntGaugeVec,
    power_usage_gauge: IntGaugeVec,
    temperature_gauge: IntGaugeVec,
    fan_speed_gauge: IntGaugeVec,
    total_memory_gauge: IntGaugeVec,
    free_memory_gauge: IntGaugeVec,
    used_memory_gauge: IntGaugeVec,
}

impl Collector {
    fn new() -> Result<Collector> {
        let nvml = NVML::init()?;

        let registry = Registry::new_custom(Some(NAMESPACE.to_string()), None).unwrap();

        // Num devices
        let num_devices_opts = Opts::new("num_devices", "Number of GPU devices");
        let num_devices_gauge = IntGauge::with_opts(num_devices_opts).unwrap();
        registry.register(Box::new(num_devices_gauge.clone())).unwrap();

        // CPU utilization
        let gpu_utilization_opts = Opts::new("gpu_utilization", "Percent of time over the past sample period during which one or more kernels were executing on the GPU device");
        let gpu_utilization_gauge = IntGaugeVec::new(gpu_utilization_opts, &LABELS).unwrap();
        registry.register(Box::new(gpu_utilization_gauge.clone())).unwrap();

        // Memory utilization
        let memory_utilization_opts = Opts::new("memory_utilization", "Percent of time over the past sample period during which global (device) memory was being read or written to.");
        let memory_utilization_gauge = IntGaugeVec::new(memory_utilization_opts, &LABELS).unwrap();
        registry.register(Box::new(memory_utilization_gauge.clone())).unwrap();

        // Power usage
        let power_usage_opts = Opts::new("power_usage_milliwatts", "Power usage of the GPU device in milliwatts");
        let power_usage_gauge = IntGaugeVec::new(power_usage_opts, &LABELS).unwrap();
        registry.register(Box::new(power_usage_gauge.clone())).unwrap();

        // Temperature
        let temperature_opts = Opts::new("temperature_celsius", "Temperature of the GPU device in celsius");
        let temperature_gauge = IntGaugeVec::new(temperature_opts, &LABELS).unwrap();
        registry.register(Box::new(temperature_gauge.clone())).unwrap();

        // Fan speed
        let fan_speed_opts = Opts::new("fanspeed_percent", "Fan speed of the GPU device as a percent of its maximum");
        let fan_speed_gauge = IntGaugeVec::new(fan_speed_opts, &LABELS).unwrap();
        registry.register(Box::new(fan_speed_gauge.clone())).unwrap();

        // Total memory
        let total_memory_opts = Opts::new("memory_total_bytes", "Total memory available by the GPU device in bytes");
        let total_memory_gauge = IntGaugeVec::new(total_memory_opts, &LABELS).unwrap();
        registry.register(Box::new(total_memory_gauge.clone())).unwrap();

        // Free memory
        let free_memory_opts = Opts::new("memory_free_bytes", "Free memory of the GPU device in bytes");
        let free_memory_gauge = IntGaugeVec::new(free_memory_opts, &LABELS).unwrap();
        registry.register(Box::new(free_memory_gauge.clone())).unwrap();

        // Used memory
        let used_memory_opts = Opts::new("memory_used_bytes", "Memory used by the GPU device in bytes");
        let used_memory_gauge = IntGaugeVec::new(used_memory_opts, &LABELS).unwrap();
        registry.register(Box::new(used_memory_gauge.clone())).unwrap();

        // Process
        let collector = Collector {
            nvml,
            registry,
            num_devices_gauge,
            gpu_utilization_gauge,
            memory_utilization_gauge,
            power_usage_gauge,
            temperature_gauge,
            fan_speed_gauge,
            total_memory_gauge,
            free_memory_gauge,
            used_memory_gauge,
        };

        Ok(collector)
    }

    fn collect(&self) -> Result<()>  {
        let num_devices = self.nvml.device_count()?;
        self.num_devices_gauge.set(num_devices.into());

        for device_num in 0..num_devices {
            let device = self.nvml.device_by_index(device_num)?;

            // Create labels
            // This only exists on Linux, so we cheat for Windows
            // let minor_number = device.minor_number()?;
            let minor_number = device_num;

            let uuid = device.uuid()?;
            let name = device.name()?;
            let labels: [&str; 3] = [&minor_number.to_string(), &uuid, &name];

            // Utilization
            if let Ok(utilization) = device.utilization_rates() {
                self.gpu_utilization_gauge.get_metric_with_label_values(&labels)?.set(utilization.gpu as i64);
                self.memory_utilization_gauge.get_metric_with_label_values(&labels)?.set(utilization.memory as i64);
            }

            // Power usage
            if let Ok(power_usage) = device.power_usage() {
                self.power_usage_gauge.get_metric_with_label_values(&labels)?.set(power_usage as i64);
            }

            // Temperature
            if let Ok(temperature) = device.temperature(TemperatureSensor::Gpu) {
                self.temperature_gauge.get_metric_with_label_values(&labels)?.set(temperature as i64);
            }

            // Fan speed
            if let Ok(fan_speed) = device.fan_speed() {
                self.fan_speed_gauge.get_metric_with_label_values(&labels)?.set(fan_speed as i64);
            }

            // Memory
            if let Ok(memory_info) = device.memory_info() {
                self.total_memory_gauge.get_metric_with_label_values(&labels)?.set(memory_info.total as i64);
                self.free_memory_gauge.get_metric_with_label_values(&labels)?.set(memory_info.free as i64);
                self.used_memory_gauge.get_metric_with_label_values(&labels)?.set(memory_info.used as i64);
            }

            // Processes
            if let Ok(processes) = device.running_compute_processes() {
                for process in processes {
                    println!("{:?}", process);
                    let accounting_stats = device.accounting_stats_for(process.pid)?;

                    if !accounting_stats.is_running {
                        continue;
                    }
                }
            }
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let collector = Collector::new()?;
    collector.collect()?;

    let mut buffer = Vec::<u8>::new();
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&collector.registry.gather(), &mut buffer).unwrap();

    println!("{}", String::from_utf8(buffer.clone()).unwrap());

    Ok(())

//    let addr = ([127, 0, 0, 1], 9898).into();
//    println!("Listening address: {:?}", addr);
//
//    // Get the first `Device` (GPU) in the system
//    let device = nvml.device_by_index(0)?;
//
//    let brand = device.brand()?; // GeForce on my system
//    let fan_speed = device.fan_speed()?; // Currently 17% on my system

//    let new_service = || {
//
//        let encoder = TextEncoder::new();
//        service_fn_ok(move |_request| {
//            NUM_DEVICES.inc();
//
//            HTTP_BODY_GAUGE.set(buffer.len() as f64);
//
//            let response = Response::builder()
//                .status(200)
//                .header(CONTENT_TYPE, encoder.format_type())
//                .body(Body::from(buffer))
//                .unwrap();
//
//            timer.observe_duration();
//
//            response
//        })
//    };
//
//    let server = Server::bind(&addr)
//        .serve(new_service)
//        .map_err(|e| eprintln!("Server error: {}", e));
//
//    hyper::rt::run(server);
}