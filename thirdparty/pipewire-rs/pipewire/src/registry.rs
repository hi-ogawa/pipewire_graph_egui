// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;
use libc::{c_char, c_void};
use std::mem;
use std::pin::Pin;
use std::{
    ffi::{CStr, CString},
    ptr,
};

use crate::{
    proxy::{Proxy, ProxyT},
    types::ObjectType,
    Error, Properties,
};
use spa::{dict::ForeignDict, prelude::*};

#[derive(Debug)]
pub struct Registry {
    ptr: ptr::NonNull<pw_sys::pw_registry>,
}

impl Registry {
    pub(crate) fn new(ptr: ptr::NonNull<pw_sys::pw_registry>) -> Self {
        Registry { ptr }
    }

    fn as_ptr(&self) -> *mut pw_sys::pw_registry {
        self.ptr.as_ptr()
    }

    // TODO: add non-local version when we'll bind pw_thread_loop_start()
    #[must_use]
    pub fn add_listener_local(&self) -> ListenerLocalBuilder {
        ListenerLocalBuilder {
            registry: self,
            cbs: ListenerLocalCallbacks::default(),
        }
    }

    pub fn bind<T: ProxyT, D: ReadableDict>(&self, object: &GlobalObject<D>) -> Result<T, Error> {
        let proxy = unsafe {
            let type_ = CString::new(object.type_.to_str()).unwrap();
            let version = object.type_.client_version();

            let proxy = spa::spa_interface_call_method!(
                self.as_ptr(),
                pw_sys::pw_registry_methods,
                bind,
                object.id,
                type_.as_ptr(),
                version,
                0
            );

            proxy
        };

        let proxy = ptr::NonNull::new(proxy.cast()).ok_or(Error::NoMemory)?;

        Proxy::new(proxy).downcast().map_err(|(_, e)| e)
    }

    /// Attempt to destroy the global object with the specified id on the remote.
    pub fn destroy_global(&self, global_id: u32) -> spa::SpaResult {
        let result = unsafe {
            spa::spa_interface_call_method!(
                self.as_ptr(),
                pw_sys::pw_registry_methods,
                destroy,
                global_id
            )
        };

        spa::SpaResult::from_c(result)
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        unsafe {
            pw_sys::pw_proxy_destroy(self.as_ptr().cast());
        }
    }
}

#[derive(Default)]
struct ListenerLocalCallbacks {
    #[allow(clippy::type_complexity)]
    global: Option<Box<dyn Fn(&GlobalObject<ForeignDict>)>>,
    global_remove: Option<Box<dyn Fn(u32)>>,
}

pub struct ListenerLocalBuilder<'a> {
    registry: &'a Registry,
    cbs: ListenerLocalCallbacks,
}

pub struct Listener {
    // Need to stay allocated while the listener is registered
    #[allow(dead_code)]
    events: Pin<Box<pw_sys::pw_registry_events>>,
    listener: Pin<Box<spa_sys::spa_hook>>,
    #[allow(dead_code)]
    data: Box<ListenerLocalCallbacks>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        spa::hook::remove(*self.listener);
    }
}

impl<'a> ListenerLocalBuilder<'a> {
    #[must_use]
    pub fn global<F>(mut self, global: F) -> Self
    where
        F: Fn(&GlobalObject<ForeignDict>) + 'static,
    {
        self.cbs.global = Some(Box::new(global));
        self
    }

    #[must_use]
    pub fn global_remove<F>(mut self, global_remove: F) -> Self
    where
        F: Fn(u32) + 'static,
    {
        self.cbs.global_remove = Some(Box::new(global_remove));
        self
    }

    #[must_use]
    pub fn register(self) -> Listener {
        unsafe extern "C" fn registry_events_global(
            data: *mut c_void,
            id: u32,
            permissions: u32,
            type_: *const c_char,
            version: u32,
            props: *const spa_sys::spa_dict,
        ) {
            let type_ = CStr::from_ptr(type_).to_str().unwrap();
            let obj = GlobalObject::new(id, permissions, type_, version, props);
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.global.as_ref().unwrap()(&obj);
        }

        unsafe extern "C" fn registry_events_global_remove(data: *mut c_void, id: u32) {
            let callbacks = (data as *mut ListenerLocalCallbacks).as_ref().unwrap();
            callbacks.global_remove.as_ref().unwrap()(id);
        }

        let e = unsafe {
            let mut e: Pin<Box<pw_sys::pw_registry_events>> = Box::pin(mem::zeroed());
            e.version = pw_sys::PW_VERSION_REGISTRY_EVENTS;

            if self.cbs.global.is_some() {
                e.global = Some(registry_events_global);
            }
            if self.cbs.global_remove.is_some() {
                e.global_remove = Some(registry_events_global_remove);
            }

            e
        };

        let (listener, data) = unsafe {
            let ptr = self.registry.as_ptr();
            let data = Box::into_raw(Box::new(self.cbs));
            let mut listener: Pin<Box<spa_sys::spa_hook>> = Box::pin(mem::zeroed());
            let listener_ptr: *mut spa_sys::spa_hook = listener.as_mut().get_unchecked_mut();

            spa::spa_interface_call_method!(
                ptr,
                pw_sys::pw_registry_methods,
                add_listener,
                listener_ptr.cast(),
                e.as_ref().get_ref(),
                data as *mut _
            );

            (listener, Box::from_raw(data))
        };

        Listener {
            events: e,
            listener,
            data,
        }
    }
}

bitflags! {
    pub struct Permission: u32 {
        const R = pw_sys::PW_PERM_R;
        const W = pw_sys::PW_PERM_W;
        const X = pw_sys::PW_PERM_X;
        const M = pw_sys::PW_PERM_M;
    }
}

#[derive(Debug)]
pub struct GlobalObject<D: ReadableDict> {
    pub id: u32,
    pub permissions: Permission,
    pub type_: ObjectType,
    pub version: u32,
    pub props: Option<D>,
}

impl GlobalObject<ForeignDict> {
    fn new(
        id: u32,
        permissions: u32,
        type_: &str,
        version: u32,
        props: *const spa_sys::spa_dict,
    ) -> Self {
        let type_ = ObjectType::from_str(type_);
        let permissions = Permission::from_bits(permissions).expect("invalid permissions");
        let props = props as *mut _;
        let props = ptr::NonNull::new(props).map(|ptr| unsafe { ForeignDict::from_ptr(ptr) });

        Self {
            id,
            permissions,
            type_,
            version,
            props,
        }
    }
}

impl<D: ReadableDict> GlobalObject<D> {
    pub fn to_owned(&self) -> GlobalObject<Properties> {
        GlobalObject {
            id: self.id,
            permissions: self.permissions,
            type_: self.type_.clone(),
            version: self.version,
            props: self
                .props
                .as_ref()
                .map(|props| Properties::from_dict(props)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_object_type() {
        assert_eq!(
            ObjectType::from_str("PipeWire:Interface:Client"),
            ObjectType::Client
        );
        assert_eq!(ObjectType::Client.to_str(), "PipeWire:Interface:Client");
        assert_eq!(ObjectType::Client.client_version(), 3);

        let o = ObjectType::Other("PipeWire:Interface:Badger".to_string());
        assert_eq!(ObjectType::from_str("PipeWire:Interface:Badger"), o);
        assert_eq!(o.to_str(), "PipeWire:Interface:Badger");
    }

    #[test]
    #[should_panic(expected = "Invalid object type")]
    fn client_version_panic() {
        let o = ObjectType::Other("PipeWire:Interface:Badger".to_string());
        assert_eq!(o.client_version(), 0);
    }
}
