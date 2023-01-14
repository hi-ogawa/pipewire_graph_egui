// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::os::raw::c_char;
use std::{
    ffi::{c_void, CStr},
    mem,
    pin::Pin,
    ptr,
};

use crate::{
    proxy::{Listener, Proxy, ProxyT},
    types::ObjectType,
};
use spa::spa_interface_call_method;

#[derive(Debug)]
pub struct Metadata {
    proxy: Proxy,
}

impl ProxyT for Metadata {
    fn type_() -> ObjectType {
        ObjectType::Metadata
    }

    fn upcast(self) -> Proxy {
        self.proxy
    }

    fn upcast_ref(&self) -> &Proxy {
        &self.proxy
    }

    unsafe fn from_proxy_unchecked(proxy: Proxy) -> Self
    where
        Self: Sized,
    {
        Self { proxy }
    }
}

impl Metadata {
    pub fn add_listener_local(&self) -> MetadataListenerLocalBuilder {
        MetadataListenerLocalBuilder {
            metadata: self,
            cbs: ListenerLocalCallbacks::default(),
        }
    }

    pub fn set_property(&self, subject: u32, key: &str, type_: Option<&str>, value: Option<&str>) {
        // Keep CStrings allocated here in order for pointers to remain valid.
        let key = CString::new(key).expect("Invalid byte in metadata key");
        let type_ = type_.map(|t| CString::new(t).expect("Invalid byte in metadata type"));
        let value = value.map(|v| CString::new(v).expect("Invalid byte in metadata value"));
        unsafe {
            spa::spa_interface_call_method!(
                self.proxy.as_ptr(),
                pw_sys::pw_metadata_methods,
                set_property,
                subject,
                key.as_ptr() as *const _,
                type_.as_deref().map_or_else(ptr::null, CStr::as_ptr) as *const _,
                value.as_deref().map_or_else(ptr::null, CStr::as_ptr) as *const _
            );
        }
    }

    pub fn clear(&self) {
        unsafe {
            spa::spa_interface_call_method!(
                self.proxy.as_ptr(),
                pw_sys::pw_metadata_methods,
                clear,
            );
        }
    }
}

pub struct MetadataListener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_metadata_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl Listener for MetadataListener {}

impl Drop for MetadataListener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

#[derive(Default)]
struct ListenerLocalCallbacks {
    #[allow(clippy::type_complexity)]
    property: Option<Box<dyn Fn(u32, Option<&str>, Option<&str>, Option<&str>) -> i32>>,
}

#[must_use]
pub struct MetadataListenerLocalBuilder<'meta> {
    metadata: &'meta Metadata,
    cbs: ListenerLocalCallbacks,
}

impl<'meta> MetadataListenerLocalBuilder<'meta> {
    /// Add property changed callback.
    ///
    /// Callback parameters: subject, key, type, value.
    ///
    /// `None` for `value` means removal of property.
    /// `None` for `key` means removal of all properties.
    pub fn property<F>(mut self, property: F) -> Self
    where
        F: Fn(u32, Option<&str>, Option<&str>, Option<&str>) -> i32 + 'static,
    {
        self.cbs.property = Some(Box::new(property));
        self
    }

    #[must_use]
    pub fn register(self) -> MetadataListener {
        unsafe extern "C" fn metadata_events_property(
            data: *mut c_void,
            subject: u32,
            key: *const c_char,
            type_: *const c_char,
            value: *const c_char,
        ) -> i32 {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            let key = if !key.is_null() {
                Some(CStr::from_ptr(key).to_string_lossy())
            } else {
                None
            };
            let type_ = if !type_.is_null() {
                Some(CStr::from_ptr(type_).to_string_lossy())
            } else {
                None
            };
            let value = if !value.is_null() {
                Some(CStr::from_ptr(value).to_string_lossy())
            } else {
                None
            };
            callbacks.property.as_ref().unwrap()(
                subject,
                key.as_deref(),
                type_.as_deref(),
                value.as_deref(),
            )
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_metadata_events>> = Box::pin(mem::zeroed());
            e.version = pw_sys::PW_VERSION_METADATA_EVENTS;

            if self.cbs.property.is_some() {
                e.property = Some(metadata_events_property);
            }

            e
        };

        let (listener, data) = unsafe {
            let metadata = &self.metadata.proxy.as_ptr();

            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();

            spa_interface_call_method!(
                metadata,
                pw_sys::pw_metadata_methods,
                add_listener,
                listener_ptr.cast(),
                e.as_ref().get_ref(),
                data as *mut _
            );

            (listener, Box::from_raw(data))
        };

        MetadataListener {
            events: e,
            listener,
            data,
        }
    }
}
