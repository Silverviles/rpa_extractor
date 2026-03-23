use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use std::sync::OnceLock;

static RENPY_AVAILABLE: OnceLock<bool> = OnceLock::new();
static PYTHON_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Initialize embedded Python: set PYTHONHOME so stdlib modules (_struct, etc.)
/// are found, and add vendored unrpyc to sys.path.
/// Must be called before any decompilation.
pub fn init_python() {
    PYTHON_INITIALIZED.get_or_init(|| {
        // Set PYTHONHOME before any Python code runs, so the embedded
        // interpreter can locate stdlib C extensions like _struct.
        if std::env::var("PYTHONHOME").is_err() {
            if let Some(home) = find_python_home() {
                std::env::set_var("PYTHONHOME", &home);
            }
        }

        Python::with_gil(|py| {
            // Find the vendor/unrpyc directory relative to the executable
            let vendor_path = find_vendor_path();
            if let Some(path) = vendor_path {
                let code = format!(
                    "import sys; sys.path.insert(0, {:?})",
                    path.to_string_lossy()
                );
                if let Ok(code_cstr) = std::ffi::CString::new(code) {
                    let _ = py.run(&code_cstr, None, None);
                }
            }
        });
        true
    });
}

/// Find the Python home directory (prefix) for MSYS2/system Python.
fn find_python_home() -> Option<String> {
    // Check common MSYS2 UCRT64 location
    let msys2_prefix = std::path::PathBuf::from("C:/msys64/ucrt64");
    if msys2_prefix.join("lib/python3.12").exists() {
        return Some(msys2_prefix.to_string_lossy().to_string());
    }

    // Try to find python on PATH and ask it for sys.prefix
    if let Ok(output) = std::process::Command::new("python")
        .args(["-c", "import sys; print(sys.prefix)"])
        .output()
    {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                return Some(prefix);
            }
        }
    }

    None
}

/// Locate the vendor/unrpyc directory. Checks:
/// 1. Next to the executable (for release builds)
/// 2. In the project root (for development)
fn find_vendor_path() -> Option<std::path::PathBuf> {
    // Try relative to executable: <exe_dir>/vendor/unrpyc
    if let Ok(exe) = std::env::current_exe() {
        let exe_vendor = exe.parent()?.join("vendor").join("unrpyc");
        if exe_vendor.join("decompiler").exists() {
            return Some(exe_vendor);
        }
    }

    // Try from working directory (development): ./vendor/unrpyc
    let cwd_vendor = std::path::PathBuf::from("vendor/unrpyc");
    if cwd_vendor.join("decompiler").exists() {
        return Some(std::fs::canonicalize(&cwd_vendor).unwrap_or(cwd_vendor));
    }

    // Try from CARGO_MANIFEST_DIR (compile-time development)
    let manifest_vendor = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("vendor")
        .join("unrpyc");
    if manifest_vendor.join("decompiler").exists() {
        return Some(manifest_vendor);
    }

    None
}

/// Check if the renpy Python module is importable.
pub fn is_renpy_available() -> bool {
    *RENPY_AVAILABLE.get_or_init(|| {
        Python::with_gil(|py| py.import("renpy").is_ok())
    })
}

const COMPILE_SCRIPT: &std::ffi::CStr = c"
import renpy
import renpy.parser
import renpy.ast
import pickle
import zlib
import struct
import io

# Initialize renpy if needed
if not hasattr(renpy, 'script'):
    try:
        renpy.game = type('obj', (object,), {'script': None})()
    except:
        pass

# Parse the .rpy source into AST
try:
    stmts = renpy.parser.parse(_filename, _source)
except Exception as e:
    raise RuntimeError(f'Parse error: {e}')

# Serialize to rpyc format
data = {
    1: stmts,
}

# Pickle the data
pickled = pickle.dumps(data, 2)

# Compress with zlib
compressed = zlib.compress(pickled, 9)

# Build rpyc file: header + slot
buf = io.BytesIO()
# RENPY RPC2 header
buf.write(b'RENPY RPC2\\x0a')
# Slot header: slot number (4), offset (4), length (4)
slot_data_offset = 11 + 12  # header + slot header
buf.write(struct.pack('<III', 1, slot_data_offset, len(compressed)))
buf.write(compressed)

_rpyc_bytes = buf.getvalue()
";

/// Compile .rpy source text into .rpyc bytes using Ren'Py's Python parser.
///
/// Returns Ok(rpyc_bytes) on success, or Err(message) if compilation fails.
/// If renpy is not installed, returns Err so the caller can fall back to removing the .rpyc.
pub fn compile_rpy_to_rpyc(rpy_source: &str, filename: &str) -> Result<Vec<u8>, String> {
    if !is_renpy_available() {
        return Err("Ren'Py Python module not available".to_string());
    }

    Python::with_gil(|py| {
        let locals = PyDict::new(py);
        locals
            .set_item("_filename", filename)
            .map_err(|e| format!("Failed to set filename: {}", e))?;
        locals
            .set_item("_source", rpy_source)
            .map_err(|e| format!("Failed to set source: {}", e))?;

        py.run(COMPILE_SCRIPT, None, Some(&locals))
            .map_err(|e| format!("Python compilation error: {}", e))?;

        let result = locals
            .get_item("_rpyc_bytes")
            .map_err(|e| format!("Failed to get result: {}", e))?
            .ok_or_else(|| "Result not found in locals".to_string())?;

        let py_bytes = result
            .downcast::<PyBytes>()
            .map_err(|e| format!("Result is not bytes: {}", e))?;

        Ok(py_bytes.as_bytes().to_vec())
    })
}

const DECOMPILE_SCRIPT: &std::ffi::CStr = c"
import pickle
import zlib
import struct

_result = ''

try:
    _raw = bytes(_rpyc_data)

    # Parse RENPY RPC2 header
    if _raw[:10] == b'RENPY RPC2':
        # Read slot: offset at bytes 15..19, length at bytes 19..23
        slot_offset = struct.unpack('<I', _raw[15:19])[0]
        slot_length = struct.unpack('<I', _raw[19:23])[0]
        compressed = _raw[slot_offset:slot_offset + slot_length]
    else:
        # Try treating entire file as zlib data
        compressed = _raw

    decompressed = zlib.decompress(compressed)

    # Use unrpyc's renpycompat to safely unpickle with Ren'Py class stubs
    from decompiler import magic, renpycompat
    data, stmts = magic.read_ast_from_data(decompressed)

    # Decompile AST to .rpy source
    from decompiler import pprint as dpprint
    from io import StringIO

    out = StringIO()
    dpprint(out, stmts)
    _result = out.getvalue()

except ImportError as e:
    # decompiler package not found - fallback
    try:
        import pprint as pp
        data = pickle.loads(decompressed, encoding='utf-8')
        _result = '# unrpyc decompiler not found: ' + str(e) + '\\n'
        _result += '# Showing raw AST structure\\n\\n'
        _result += pp.pformat(data, width=100)
    except Exception as e2:
        _result = '# Failed to decompile: ' + str(e) + ', fallback also failed: ' + str(e2)

except Exception as e:
    _result = '# Failed to read rpyc file: ' + str(e)
";

/// Decompile .rpyc binary data into readable .rpy source text.
///
/// Uses the vendored unrpyc decompiler. Falls back to raw AST dump if unavailable.
/// Returns the decompiled text or an error message as text (never fails hard).
pub fn decompile_rpyc(rpyc_data: &[u8]) -> String {
    init_python();

    Python::with_gil(|py| {
        let locals = PyDict::new(py);
        let py_bytes = PyBytes::new(py, rpyc_data);
        if locals.set_item("_rpyc_data", py_bytes).is_err() {
            return "# Failed to pass data to Python".to_string();
        }

        if let Err(e) = py.run(DECOMPILE_SCRIPT, None, Some(&locals)) {
            return format!("# Python decompilation error: {}", e);
        }

        match locals.get_item("_result") {
            Ok(Some(result)) => result.extract::<String>().unwrap_or_default(),
            _ => "# Failed to get decompilation result".to_string(),
        }
    })
}

/// Given a .rpy filename, return the corresponding .rpyc filename.
/// Returns None if the input doesn't end with .rpy.
pub fn rpyc_counterpart(rpy_filename: &str) -> Option<String> {
    if rpy_filename.ends_with(".rpy") {
        Some(format!("{}c", rpy_filename))
    } else {
        None
    }
}

/// Given a .rpyc filename, return the corresponding .rpy filename.
pub fn rpy_counterpart(rpyc_filename: &str) -> Option<String> {
    if rpyc_filename.ends_with(".rpyc") {
        Some(rpyc_filename[..rpyc_filename.len() - 1].to_string())
    } else {
        None
    }
}
