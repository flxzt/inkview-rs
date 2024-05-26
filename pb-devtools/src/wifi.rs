use crate::error::DevtoolsError;
use inkview::bindings::Inkview;
use std::ffi::{c_char, CStr};

pub fn wifi_activate(iv: &Inkview, show_hourglass: bool) -> Result<(), DevtoolsError> {
    if wifi_check_connected(iv)? {
        return Ok(());
    }
    try_connect(iv, show_hourglass)?;
    if wifi_check_connected(iv)? {
        return Ok(());
    }
    Ok(())
}

pub fn wifi_deactivate(iv: &Inkview) -> Result<(), DevtoolsError> {
    if !wifi_check_connected(iv)? {
        return Ok(());
    }
    let res = unsafe { iv.NetDisconnect() };
    if res != 0 {
        return Err(DevtoolsError::WifiDisconnect);
    }
    Ok(())
}

pub fn wifi_keepalive(iv: &Inkview) -> Result<(), DevtoolsError> {
    wifi_activate(iv, false)
}

pub fn wifi_check_connected(iv: &Inkview) -> Result<bool, DevtoolsError> {
    let netinfo = unsafe {
        iv.NetInfo().as_mut().ok_or(DevtoolsError::Inkview {
            code: None,
            msg: "NetInfo returned NULL".to_string(),
        })?
    };
    let is_connected = netinfo.connected != 0;
    Ok(is_connected)
}

fn try_connect(iv: &Inkview, show_hourglass: bool) -> Result<Option<String>, DevtoolsError> {
    let network_name = std::ptr::null() as *const c_char;
    let show_hourglass = if show_hourglass { 1 } else { 0 };
    let res = unsafe { iv.NetConnect2(network_name, show_hourglass) };
    if res != 0 {
        return Err(DevtoolsError::WifiConnect);
    }
    if network_name.is_null() {
        return Ok(None);
    }
    let network_name = unsafe { CStr::from_ptr(network_name).to_owned() };
    Ok(Some(network_name.to_string_lossy().into()))
}
