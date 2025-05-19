use crate::error::DevtoolsError;
use inkview::bindings::Inkview;

pub fn report_status(_iv: &'static Inkview) -> Result<String, DevtoolsError> {
    let mut status = String::new();
    status += "status!";
    Ok(status)
}
