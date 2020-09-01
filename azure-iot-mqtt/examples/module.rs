// An example module client.
//
// - Connects to an Azure IoT Hub using bare TLS or WebSockets.
// - Responds to direct method requests by returning the same payload.
// - Reports twin state once at start, then updates it periodically after.
//
//
// Example:
//
//     cargo run --example module -- --device-id <> --module-id <> --iothub-hostname <> --sas-token <> --use-websocket --will 'azure-iot-mqtt client unexpectedly disconnected'

mod common;

#[derive(Debug, structopt::StructOpt)]
#[structopt(group = common::authentication_group())]
struct Options {
	#[structopt(help = "Device ID", long = "device-id")]
	device_id: String,

	#[structopt(help = "Module ID", long = "module-id")]
	module_id: String,

	#[structopt(help = "IoT Hub hostname (eg foo.azure-devices.net)", long = "iothub-hostname")]
	iothub_hostname: String,

	#[structopt(help = "SAS token for token authentication", long = "sas-token", group = "authentication")]
	sas_token: Option<String>,

	#[structopt(
		help = "Path of certificate file (PKCS #12) for certificate authentication",
		long = "certificate-file",
		group = "authentication",
		requires = "certificate-file-password",
		parse(from_os_str),
	)]
	certificate_file: Option<std::path::PathBuf>,

	#[structopt(
		help = "Password to decrypt certificate file for certificate authentication",
		long = "certificate-file-password",
		requires = "certificate-file",
	)]
	certificate_file_password: Option<String>,

	#[structopt(help = "Whether to use websockets or bare TLS to connect to the Iot Hub", long = "use-websocket")]
	use_websocket: bool,

	#[structopt(help = "Will message to publish if this client disconnects unexpectedly", long = "will")]
	will: Option<String>,

	#[structopt(
		help = "Maximum back-off time between reconnections to the server, in seconds.",
		long = "max-back-off",
		default_value = "30",
		parse(try_from_str = common::duration_from_secs_str),
	)]
	max_back_off: std::time::Duration,

	#[structopt(
		help = "Keep-alive time advertised to the server, in seconds.",
		long = "keep-alive",
		default_value = "5",
		parse(try_from_str = common::duration_from_secs_str),
	)]
	keep_alive: std::time::Duration,

	#[structopt(
		help = "Interval at which the client reports its twin state to the server, in seconds.",
		long = "report-twin-state-period",
		default_value = "5",
		parse(try_from_str = common::duration_from_secs_str),
	)]
	report_twin_state_period: std::time::Duration,
}

#[tokio::main]
async fn main() {
	env_logger::Builder::from_env(env_logger::Env::new().filter_or("AZURE_IOT_MQTT_LOG", "mqtt3=debug,mqtt3::logging=trace,azure_iot_mqtt=debug,module=info")).init();

	let Options {
		device_id,
		module_id,
		iothub_hostname,
		sas_token,
		certificate_file,
		certificate_file_password,
		use_websocket,
		will,
		max_back_off,
		keep_alive,
		report_twin_state_period,
	} = structopt::StructOpt::from_args();

	let authentication = common::parse_authentication(&device_id, None, None, sas_token, certificate_file, certificate_file_password);

	let runtime = tokio::runtime::Runtime::new().expect("couldn't initialize tokio runtime");
	let runtime_handle = runtime.handle().clone();

	let mut client = azure_iot_mqtt::module::Client::new(
		iothub_hostname,
		&device_id,
		&module_id,
		authentication,
		if use_websocket { azure_iot_mqtt::Transport::WebSocket } else { azure_iot_mqtt::Transport::Tcp },

		will.map(Into::into),

		max_back_off,
		keep_alive,
	).await.expect("could not create client");

	common::spawn_background_tasks(
		&runtime_handle,
		client.inner().shutdown_handle(),
		client.report_twin_state_handle(),
		report_twin_state_period,
	);

	let direct_method_response_handle = client.direct_method_response_handle();

	use futures_util::StreamExt;

	while let Some(message) = client.next().await {
		let message = message.unwrap();

		log::info!("received message {:?}", message);

		if let azure_iot_mqtt::module::Message::DirectMethod { name, payload, request_id } = message {
			log::info!("direct method {:?} invoked with payload {:?}", name, payload);

			let mut direct_method_response_handle = direct_method_response_handle.clone();

			// Respond with status 200 and same payload
			runtime_handle.spawn(async move {
				let result = direct_method_response_handle.respond(request_id.clone(), azure_iot_mqtt::Status::Ok, payload).await;
				let () = result.expect("couldn't send direct method response");
				log::info!("Responded to request {}", request_id);
			});
		}
	}
}
