use std::io::Read;

pub(crate) fn authentication_group() -> structopt::clap::ArgGroup<'static> {
	structopt::clap::ArgGroup::with_name("authentication").required(true)
}

pub(crate) fn duration_from_secs_str(s: &str) -> Result<std::time::Duration, <u64 as std::str::FromStr>::Err> {
	Ok(std::time::Duration::from_secs(s.parse()?))
}

pub(crate) fn spawn_background_tasks(
	runtime_handle: &tokio::runtime::Handle,
	shutdown_handle: Result<mqtt3::ShutdownHandle, mqtt3::ShutdownError>,
	mut report_twin_state_handle: azure_iot_mqtt::ReportTwinStateHandle,
	report_twin_state_period: std::time::Duration,
) {
	let mut shutdown_handle = shutdown_handle.expect("couldn't get shutdown handle");
	runtime_handle.spawn(async move {
		let () = tokio::signal::ctrl_c().await.expect("couldn't get Ctrl-C notification");
		let result = shutdown_handle.shutdown().await;
		let () = result.expect("couldn't send shutdown notification");
	});

	runtime_handle.spawn(async move {
		use futures_util::StreamExt;

		let result = report_twin_state_handle.report_twin_state(azure_iot_mqtt::ReportTwinStateRequest::Replace(
			vec![("start-time".to_string(), chrono::Utc::now().to_string().into())].into_iter().collect()
		)).await;
		let () = result.expect("couldn't report initial twin state");

		let mut interval = tokio::time::interval(report_twin_state_period);
		while interval.next().await.is_some() {
			let result = report_twin_state_handle.report_twin_state(azure_iot_mqtt::ReportTwinStateRequest::Patch(
				vec![("current-time".to_string(), chrono::Utc::now().to_string().into())].into_iter().collect()
			)).await;

			let () = result.expect("couldn't report twin state patch");
		}
	});
}
