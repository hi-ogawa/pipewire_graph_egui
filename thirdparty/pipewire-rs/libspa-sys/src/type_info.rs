use super::*;

extern "C" {
    pub static libspa_rs_types: *const spa_type_info;
    pub static libspa_rs_type_direction: *const spa_type_info;
    pub static libspa_rs_type_choice: *const spa_type_info;
    pub static libspa_rs_type_device_event_id: *const spa_type_info;
    pub static libspa_rs_type_device_event: *const spa_type_info;
    pub static libspa_rs_type_io: *const spa_type_info;
    pub static libspa_rs_type_node_event_id: *const spa_type_info;
    pub static libspa_rs_type_node_event: *const spa_type_info;
    pub static libspa_rs_type_node_command_id: *const spa_type_info;
    pub static libspa_rs_type_node_command: *const spa_type_info;
    pub static libspa_rs_type_data_type: *const spa_type_info;
    pub static libspa_rs_type_meta_type: *const spa_type_info;
    pub static libspa_rs_type_control: *const spa_type_info;
    pub static libspa_rs_type_param: *const spa_type_info;
    pub static libspa_rs_type_prop_float_array: *const spa_type_info;
    pub static libspa_rs_type_prop_channel_map: *const spa_type_info;
    pub static libspa_rs_type_prop_iec958_codec: *const spa_type_info;
    pub static libspa_rs_type_param_bitorder: *const spa_type_info;
    pub static libspa_rs_type_props: *const spa_type_info;
    pub static libspa_rs_type_prop_info: *const spa_type_info;
    pub static libspa_rs_type_param_meta: *const spa_type_info;
    pub static libspa_rs_type_param_io: *const spa_type_info;
    pub static libspa_rs_type_media_type: *const spa_type_info;
    pub static libspa_rs_type_media_subtype: *const spa_type_info;
    pub static libspa_rs_type_format: *const spa_type_info;
    pub static libspa_rs_type_param_buffers: *const spa_type_info;
    pub static libspa_rs_type_param_availability: *const spa_type_info;
    pub static libspa_rs_type_param_profile: *const spa_type_info;
    pub static libspa_rs_type_param_port_config_mode: *const spa_type_info;
    pub static libspa_rs_type_param_port_config: *const spa_type_info;
    pub static libspa_rs_type_param_route: *const spa_type_info;
    pub static libspa_rs_type_profiler: *const spa_type_info;
    pub static libspa_rs_type_param_latency: *const spa_type_info;
    pub static libspa_rs_type_param_process_latency: *const spa_type_info;
    pub static libspa_rs_type_audio_format: *const spa_type_info;
    pub static libspa_rs_type_audio_flags: *const spa_type_info;
    pub static libspa_rs_type_audio_channel: *const spa_type_info;
    pub static libspa_rs_type_audio_iec958_codec: *const spa_type_info;
    pub static libspa_rs_type_bluetooth_audio_codec: *const spa_type_info;
    pub static libspa_rs_type_video_format: *const spa_type_info;

    pub fn libspa_rs_debug_type_find(
        info: *const spa_type_info,
        type_: u32,
    ) -> *const spa_type_info;
}

#[cfg(test)]
mod test {
    use crate::{libspa_rs_type_media_type, SPA_MEDIA_TYPE_audio};

    use std::ffi;

    #[test]
    fn test_libspa_rs_debug_type_find() {
        unsafe {
            let type_info =
                super::libspa_rs_debug_type_find(libspa_rs_type_media_type, SPA_MEDIA_TYPE_audio);
            assert_eq!(
                ffi::CStr::from_ptr((*type_info).name),
                ffi::CString::new("Spa:Enum:MediaType:audio")
                    .unwrap()
                    .as_ref()
            );
        }
    }
}
