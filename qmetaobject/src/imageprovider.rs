use std::ffi::{c_void, CStr};

use cpp::cpp;

use qttypes::QString;

use crate::QmlEngine;

cpp!({
    #include <QtQml/QQmlEngine>
    #include <QtQml/QQmlEngineExtensionPlugin>
    #include <QtQuick/QQuickImageProvider>

    #include <QtGui/QImage>
    #include <QtGui/QPainter>

    #include <stdlib.h>
    #include <stdio.h>
    #include <string>

    typedef struct {
        unsigned char *data;
        unsigned int len;
    } tuple_t;

    class ProxyImageProvider : public QQuickImageProvider {

        void* data = nullptr;
        tuple_t (*fcn_read)(void *data, const char *id) = nullptr;
        void (*fcn_free)(unsigned char *data, unsigned int len) = nullptr;

    public:
        ProxyImageProvider(void *data_ptr, void *fcn_get_ptr, void *fcn_free_ptr)
         : QQuickImageProvider(QQuickImageProvider::Pixmap) {
            data = data_ptr;
            fcn_read = (tuple_t (*)(void *data, const char *id)) fcn_get_ptr;
            fcn_free = (void (*)(unsigned char *data, unsigned int len)) fcn_free_ptr;
        }

        QPixmap requestPixmap(const QString &id, QSize *size, const QSize &requestedSize) override {
            std::string std_id = id.toStdString();

            tuple_t t = fcn_read(data, std_id.c_str());

            QPixmap pixmap;
            int rz = pixmap.loadFromData(t.data, t.len);

            fcn_free(t.data, t.len);

            return pixmap;
        }
    };
});

#[repr(C)]
struct FfiTuple {
    data: *mut char,
    len: u32,
}

unsafe extern "C" fn get_data(ptr: *mut c_void, id: *mut char) -> FfiTuple {
    // Convert to safer interpretation
    let id: &CStr = CStr::from_ptr(id as *const i8);
    let id: &str = id.to_str().unwrap();

    // Call user-defined function
    let fcn: fn(id: &str) -> Vec<u8> = std::mem::transmute_copy(&ptr);
    let data = fcn(id);

    // Convert to FFI format
    let mut boxed_slice: Box<[u8]> = data.into_boxed_slice();

    let array: *mut u8 = boxed_slice.as_mut_ptr();
    let array_len: usize = boxed_slice.len();

    std::mem::forget(boxed_slice); // Prevent box destructor call

    FfiTuple { data: array as *mut char, len: array_len as u32 }
}

unsafe extern "C" fn free_data(ptr: *mut char, len: u32) {
    let b = Box::from_raw(std::slice::from_raw_parts_mut(ptr, len as usize));
    // At the end memory will be free
}

impl QmlEngine {
    pub fn add_proxy_image_provider(
        &mut self,
        name: QString,
        provider_function: fn(id: &str) -> Vec<u8>,
    ) {
        let fcn_call: unsafe extern "C" fn(ptr: *mut c_void, id: *mut char) -> FfiTuple = get_data;
        let fcn_free: unsafe extern "C" fn(ptr: *mut char, len: u32) = free_data;

        cpp!(unsafe [
            self as "QmlEngineHolder *",
            name as "QString",
            provider_function as "void *",
            fcn_call as "void *",
            fcn_free as "void *"
        ] {
            self->engine->addImageProvider(name, new ProxyImageProvider(provider_function, fcn_call, fcn_free));
        })
    }
}
