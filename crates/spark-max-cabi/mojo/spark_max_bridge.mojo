# Spark Modular MAX C-ABI Native Dynamic Bridge
# Pure compiled C-ABI shared library for direct Rust invocation

@export("spark_max_version")
def spark_max_version() -> Int32:
    return 1

@export("spark_max_session_create")
def spark_max_session_create(device_id: Int32) -> Int32:
    if device_id < 0:
        return -1
    return 1001 + device_id

@export("spark_max_session_destroy")
def spark_max_session_destroy(session_id: Int32) -> Int32:
    if session_id > 0:
        return 0
    return -1

@export("spark_max_compute_scalar")
def spark_max_compute_scalar(session_id: Int32, input_val: Float32) -> Float32:
    return input_val * 2.5 + Float32(session_id)
