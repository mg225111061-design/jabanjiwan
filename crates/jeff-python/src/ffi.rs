//! CPython C-API bindings (only built with `--features embed`). Minimal, safe-ish wrappers
//! that initialise the interpreter once, run/eval code, propagate Python exceptions into
//! [`PyError`], and marshal scalar results — each returned as [`Trusted`] (P1 boundary).
//!
//! Reference-counting: every `PyRun_String`/`PyObject_Str` result is a new reference that we
//! `Py_DecRef` before returning; `PyImport_AddModule`/`PyModule_GetDict` are borrowed.

use crate::{PyError, Trusted};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::sync::Once;

type Obj = *mut c_void;

const PY_FILE_INPUT: c_int = 257;
const PY_EVAL_INPUT: c_int = 258;

extern "C" {
    fn Py_IsInitialized() -> c_int;
    fn Py_InitializeEx(install_signal_handlers: c_int);
    fn PyImport_AddModule(name: *const c_char) -> Obj; // borrowed
    fn PyModule_GetDict(module: Obj) -> Obj; // borrowed
    fn PyRun_StringFlags(
        s: *const c_char,
        start: c_int,
        globals: Obj,
        locals: Obj,
        flags: *mut c_void,
    ) -> Obj; // new ref or null
    fn PyErr_Occurred() -> Obj; // borrowed
    fn PyErr_Fetch(ptype: *mut Obj, pvalue: *mut Obj, ptb: *mut Obj);
    fn PyObject_Str(o: Obj) -> Obj; // new ref
    fn PyUnicode_AsUTF8(o: Obj) -> *const c_char; // borrowed into o
    fn PyLong_AsLongLong(o: Obj) -> i64;
    fn PyFloat_AsDouble(o: Obj) -> f64;
    fn Py_DecRef(o: Obj);
    fn PyEval_SaveThread() -> Obj; // drop the GIL, return saved thread state
    fn PyGILState_Ensure() -> c_int; // acquire the GIL on this thread
    fn PyGILState_Release(state: c_int);
}

static INIT: Once = Once::new();

/// Initialise the embedded interpreter exactly once (idempotent). Signal handlers are not
/// installed (`Py_InitializeEx(0)`) so JEFF keeps control of the process. After init we
/// **release the GIL** (`PyEval_SaveThread`) so any native thread can later acquire it via
/// `PyGILState_Ensure` — embedding from arbitrary threads is layer 7 of the directive.
pub fn ensure_init() {
    INIT.call_once(|| unsafe {
        if Py_IsInitialized() == 0 {
            Py_InitializeEx(0);
            let _ = PyEval_SaveThread();
        }
    });
}

/// Run `f` with the GIL held on the current thread (acquire on entry, release on exit).
/// Every operation that touches a `PyObject` goes through here, so calls are safe from any
/// thread (e.g. the parallel test harness).
fn with_gil<R>(f: impl FnOnce() -> R) -> R {
    ensure_init();
    unsafe {
        let state = PyGILState_Ensure();
        let r = f();
        PyGILState_Release(state);
        r
    }
}

/// Fetch and clear the current Python exception as a string. Caller must hold the GIL (we
/// are single-threaded after init). Decrefs the fetched objects.
unsafe fn take_error() -> String {
    let mut t: Obj = ptr::null_mut();
    let mut v: Obj = ptr::null_mut();
    let mut tb: Obj = ptr::null_mut();
    PyErr_Fetch(&mut t, &mut v, &mut tb);
    let msg = if !v.is_null() {
        let s = PyObject_Str(v);
        let out = if s.is_null() {
            "<unprintable exception>".to_string()
        } else {
            let c = PyUnicode_AsUTF8(s);
            let m = if c.is_null() {
                "<unprintable exception>".to_string()
            } else {
                CStr::from_ptr(c).to_string_lossy().into_owned()
            };
            Py_DecRef(s);
            m
        };
        out
    } else {
        "<unknown exception>".to_string()
    };
    if !t.is_null() {
        Py_DecRef(t);
    }
    if !v.is_null() {
        Py_DecRef(v);
    }
    if !tb.is_null() {
        Py_DecRef(tb);
    }
    msg
}

/// `__main__`'s globals dict (borrowed). Interpreter must be initialised.
unsafe fn main_globals() -> Obj {
    let main = PyImport_AddModule(c"__main__".as_ptr());
    PyModule_GetDict(main)
}

/// Run `code` (statements) in `__main__`. Propagates a raised exception as [`PyError`].
pub fn run(code: &str) -> Result<(), PyError> {
    exec(code)
}

/// Execute statements (alias of [`run`]) — `Py_file_input`.
pub fn exec(code: &str) -> Result<(), PyError> {
    let c = CString::new(code).map_err(|_| PyError("code contains a NUL byte".into()))?;
    with_gil(|| unsafe {
        let g = main_globals();
        let res = PyRun_StringFlags(c.as_ptr(), PY_FILE_INPUT, g, g, ptr::null_mut());
        if res.is_null() {
            return Err(PyError(take_error()));
        }
        Py_DecRef(res);
        Ok(())
    })
}

/// Evaluate an expression, returning the raw new-reference result or a propagated error.
/// GIL must already be held (call inside [`with_gil`]).
unsafe fn eval_obj(expr: &str) -> Result<Obj, PyError> {
    let c = CString::new(expr).map_err(|_| PyError("expr contains a NUL byte".into()))?;
    let g = main_globals();
    let res = PyRun_StringFlags(c.as_ptr(), PY_EVAL_INPUT, g, g, ptr::null_mut());
    if res.is_null() {
        return Err(PyError(take_error()));
    }
    Ok(res)
}

/// Evaluate a Python expression to an `i64` (trusted — it came from Python).
pub fn eval_i64(expr: &str) -> Result<Trusted<i64>, PyError> {
    with_gil(|| unsafe {
        let obj = eval_obj(expr)?;
        let v = PyLong_AsLongLong(obj);
        Py_DecRef(obj);
        if !PyErr_Occurred().is_null() {
            let e = take_error();
            return Err(PyError(format!("result is not an integer: {e}")));
        }
        Ok(Trusted::new(v))
    })
}

/// Evaluate a Python expression to an `f64` (trusted).
pub fn eval_f64(expr: &str) -> Result<Trusted<f64>, PyError> {
    with_gil(|| unsafe {
        let obj = eval_obj(expr)?;
        let v = PyFloat_AsDouble(obj);
        Py_DecRef(obj);
        if !PyErr_Occurred().is_null() {
            let e = take_error();
            return Err(PyError(format!("result is not a float: {e}")));
        }
        Ok(Trusted::new(v))
    })
}

/// Evaluate a Python expression and take `str(result)` (trusted).
pub fn eval_string(expr: &str) -> Result<Trusted<String>, PyError> {
    with_gil(|| unsafe {
        let obj = eval_obj(expr)?;
        let s = PyObject_Str(obj);
        Py_DecRef(obj);
        if s.is_null() {
            return Err(PyError(take_error()));
        }
        let c = PyUnicode_AsUTF8(s);
        if c.is_null() {
            Py_DecRef(s);
            return Err(PyError("could not decode str()".into()));
        }
        let out = CStr::from_ptr(c).to_string_lossy().into_owned();
        Py_DecRef(s);
        Ok(Trusted::new(out))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Provenance;

    #[test]
    fn embeds_and_evaluates() {
        // The actual CPython interpreter runs and returns a value across the FFI.
        let r = eval_i64("sum([1, 2, 3]) * 7").unwrap();
        assert_eq!(*r.get(), 42);
        assert_eq!(r.provenance(), Provenance::Trusted); // never Verified (P1)
        assert!((eval_f64("3.0 ** 0.5").unwrap().get() - 1.7320508).abs() < 1e-6);
        assert_eq!(eval_string("''.join(reversed('jeff'))").unwrap().get(), "ffej");
    }

    #[test]
    fn python_exception_propagates() {
        // A raised exception becomes a Rust Err carrying the message (not a crash).
        let e = exec("raise ValueError('boom from python')").unwrap_err();
        assert!(e.0.contains("boom from python"), "got: {}", e.0);
        // a NameError in an expression propagates too
        let e2 = eval_i64("undefined_name + 1").unwrap_err();
        assert!(e2.0.to_lowercase().contains("undefined_name") || e2.0.contains("NameError"));
    }

    #[test]
    fn state_persists_across_calls() {
        // run() mutates __main__; a later eval sees it (one live interpreter).
        run("import math\n_jx = math.factorial(6)").unwrap();
        assert_eq!(*eval_i64("_jx").unwrap().get(), 720);
    }

    #[test]
    fn drives_a_stdlib_pipeline() {
        // Drive real Python library code (statistics) and read the result back, trusted.
        run("import statistics as st\n_vals = [2,4,4,4,5,5,7,9]\n_sd = st.pstdev(_vals)").unwrap();
        let sd = eval_f64("_sd").unwrap();
        assert!((sd.get() - 2.0).abs() < 1e-9, "pstdev=2.0, got {}", sd.get());
        assert_eq!(sd.provenance(), Provenance::Trusted);
    }
}

#[cfg(test)]
mod numpy_tests {
    use super::*;
    #[test]
    fn drives_numpy_if_present() {
        match run("import numpy as np") {
            Ok(()) => {
                let v = eval_i64("int(np.arange(100).sum())").unwrap();
                eprintln!("[numpy] JEFF drove numpy: arange(100).sum() = {}", v.get());
                assert_eq!(*v.get(), 4950);
                let f = eval_f64("float(np.linalg.norm(np.array([3.0,4.0])))").unwrap();
                eprintln!("[numpy] np.linalg.norm([3,4]) = {}", f.get());
                assert!((f.get() - 5.0).abs() < 1e-12);
            }
            Err(e) => eprintln!("[numpy] NOT importable in embedded interp: {}", e.0),
        }
    }
}

/// Stage 13 layer 3 (zero-copy array interchange) — minimal read-side via the CPython buffer
/// protocol (PEP 3118). Reads a contiguous float64 buffer from a Python object WITHOUT
/// copying (the slice points into Python's memory), for a bit-exact cross-check.
#[cfg(feature = "embed")]
pub mod bufproto {
    use super::*;
    // Fields after `buf`/`len` are unread here but MUST be present for the correct CPython
    // `Py_buffer` C ABI layout (PyObject_GetBuffer writes the whole struct).
    #[allow(dead_code)]
    #[repr(C)]
    struct PyBuffer {
        buf: *mut c_void,
        obj: Obj,
        len: isize,
        itemsize: isize,
        readonly: c_int,
        ndim: c_int,
        format: *mut c_char,
        shape: *mut isize,
        strides: *mut isize,
        suboffsets: *mut isize,
        internal: *mut c_void,
    }
    extern "C" {
        fn PyObject_GetBuffer(obj: Obj, view: *mut PyBuffer, flags: c_int) -> c_int;
        fn PyBuffer_Release(view: *mut PyBuffer);
    }
    const PYBUF_SIMPLE: c_int = 0;

    /// Sum a Python object's float64 buffer by reading it zero-copy (no Vec copy of the data),
    /// plus report `(addr, nbytes)` so the caller can prove it is the same memory.
    pub fn f64_buffer_zerocopy_sum(expr: &str) -> Result<(f64, usize, usize), PyError> {
        with_gil(|| unsafe {
            let obj = eval_obj(expr)?; // new ref to the array object
            let mut view: PyBuffer = std::mem::zeroed();
            let rc = PyObject_GetBuffer(obj, &mut view, PYBUF_SIMPLE);
            if rc != 0 {
                Py_DecRef(obj);
                return Err(PyError(take_error()));
            }
            let n = (view.len as usize) / 8;
            let ptr = view.buf as *const f64;
            let slice = std::slice::from_raw_parts(ptr, n); // zero-copy view into Python memory
            let sum: f64 = slice.iter().sum();
            let addr = view.buf as usize;
            let nbytes = view.len as usize;
            PyBuffer_Release(&mut view);
            Py_DecRef(obj);
            Ok((sum, addr, nbytes))
        })
    }
}

#[cfg(all(test, feature = "embed"))]
mod buffer_tests {
    use super::*;
    #[test]
    fn numpy_zero_copy_buffer_roundtrip() {
        if run("import numpy as np").is_err() {
            eprintln!("[numpy] not present");
            return;
        }
        // create a float64 array in Python; read its buffer zero-copy in Rust.
        run("_arr = np.arange(1000, dtype=np.float64) * 2.5").unwrap();
        let (sum, addr, nbytes) = bufproto::f64_buffer_zerocopy_sum("_arr").unwrap();
        // numpy's own sum (independent) — bit-exact match.
        let np_sum = eval_f64("float(_arr.sum())").unwrap();
        let np_addr = eval_i64("_arr.__array_interface__['data'][0]").unwrap();
        eprintln!("[numpy] zero-copy: rust_sum={sum} np_sum={} addr=0x{addr:x} np_addr=0x{:x} nbytes={nbytes}", np_sum.get(), *np_addr.get());
        assert_eq!(sum, *np_sum.get(), "zero-copy buffer sum must match numpy bit-for-bit");
        assert_eq!(addr, *np_addr.get() as usize, "same memory address ⇒ NOT copied (zero-copy)");
        assert_eq!(nbytes, 8000);
    }
}
