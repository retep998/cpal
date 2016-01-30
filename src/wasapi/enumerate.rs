use super::winapi;
use super::ole32;
use super::com;
use super::Endpoint;
use super::check_result;
use super::wio::com::ComPtr;

use std::mem;
use std::ptr;

lazy_static! {
    static ref ENUMERATOR: Enumerator = {
        // COM initialization is thread local, but we only need to have COM initialized in the
        // thread we create the objects in
        com::com_initialized();

        // building the devices enumerator object
        unsafe {
            let mut enumerator = mem::uninitialized();
            
            let hresult = ole32::CoCreateInstance(&winapi::CLSID_MMDeviceEnumerator,
                                                  ptr::null_mut(), winapi::CLSCTX_ALL,
                                                  &winapi::IID_IMMDeviceEnumerator,
                                                  &mut enumerator);

            check_result(hresult).unwrap();
            Enumerator(ComPtr::new(enumerator as *mut _))
        }
    };
}

/// RAII object around `winapi::IMMDeviceEnumerator`.
struct Enumerator(ComPtr<winapi::IMMDeviceEnumerator>);

unsafe impl Send for Enumerator {}
unsafe impl Sync for Enumerator {}

/// WASAPI implementation for `EndpointsIterator`.
pub struct EndpointsIterator {
    collection: ComPtr<winapi::IMMDeviceCollection>,
    total_count: u32,
    next_item: u32,
}

unsafe impl Send for EndpointsIterator {}
unsafe impl Sync for EndpointsIterator {}


impl Default for EndpointsIterator {
    fn default() -> EndpointsIterator {
        unsafe {
            let mut collection: *mut winapi::IMMDeviceCollection = mem::uninitialized();
            // can fail because of wrong parameters (should never happen) or out of memory
            check_result(ENUMERATOR.0.as_mut().EnumAudioEndpoints(winapi::eRender,
                                                            winapi::DEVICE_STATE_ACTIVE,
                                                            &mut collection))
                                                            .unwrap();

            let mut collection = ComPtr::new(collection);
            let mut count = mem::uninitialized();
            // can fail if the parameter is null, which should never happen
            check_result(collection.GetCount(&mut count)).unwrap();

            EndpointsIterator {
                collection: collection,
                total_count: count,
                next_item: 0,
            }
        }
    }
}

impl Iterator for EndpointsIterator {
    type Item = Endpoint;

    fn next(&mut self) -> Option<Endpoint> {
        if self.next_item >= self.total_count {
            return None;
        }

        unsafe {
            let mut device = mem::uninitialized();
            // can fail if out of range, which we just checked above
            check_result(self.collection.Item(self.next_item, &mut device)).unwrap();

            self.next_item += 1;
            Some(Endpoint::from_immdevice(ComPtr::new(device)))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let num = self.total_count - self.next_item;
        let num = num as usize;
        (num, Some(num))
    }
}

pub fn get_default_endpoint() -> Option<Endpoint> {
    unsafe {
        let mut device = mem::uninitialized();
        let hres = ENUMERATOR.0.as_mut().GetDefaultAudioEndpoint(winapi::eRender,
                                                           winapi::eConsole, &mut device);

        if let Err(_err) = check_result(hres) {
            return None;        // TODO: check specifically for `E_NOTFOUND`, and panic otherwise
        }

        Some(Endpoint::from_immdevice(ComPtr::new(device)))
    }
}
