# IBMTTS Host 32-bit

This project provides a robust, asynchronous 32-bit bridge for the IBMTTS (IBM Text-to-Speech) ECI engine. It is designed to allow modern 64-bit applications (or other 32-bit clients) to communicate with the legacy 32-bit `eci.dll` through a high-performance Inter-Process Communication (IPC) layer.

---

## 🏗 System Architecture

The host is built with a multi-threaded, asynchronous architecture using Rust. It acts as a middleware that manages the lifecycle of the TTS engine and handles requests from multiple clients.

### Core Components

1. **Named Pipes (IPC Layer):**
The primary communication channel. The host creates a server at `\\.\pipe\ibmtts_host32` using **Message Mode**. Unlike byte streams, message mode ensures that a `write` from the client corresponds exactly to a `read` on the server, preserving packet boundaries.
* **Overlapped I/O:** Uses Windows Completion Routines (APCs) to handle communication without blocking the main thread.


2. **The Handler System:**
Located in `worker.rs`, this system dispatches incoming binary packets to specific Rust functions. It uses a `Registry` (a HashMap of IDs to function pointers) to route requests efficiently.
3. **The Worker:**
The worker logic translates generic IPC requests into specific calls to the IBMTTS library. It handles data types like signed/unsigned integers, byte strings, and UTF-8 text.
4. **LibLoader:**
A dynamic wrapper around `libloading`. It uses a custom macro `define_eci_api!` to map the C-style exports of `eci.dll` into a type-safe Rust structure at runtime.

---



## 🏗 Building and testing

cargo build --target=i686-pc-windows-msvc
or
cargo run --target=i686-pc-windows-msvc
or
cargo build --release --lib --target i686-pc-windows-msvc

---

## ⚡ Shared Memory & Events (Callback Mechanism)

In this architecture, the **Shared Memory** is the high-speed highway used exclusively for audio data and real-time callbacks. While the Named Pipe handles commands (like "set volume"), the Shared Memory handles the heavy lifting of audio samples to avoid the overhead of the IPC pipe.

IBMTTS requires a callback function to return audio samples. Since callbacks happen on the Host's side, we use **Shared Memory** to stream high-bandwidth audio data back to the client without the overhead of pipe copies.

* **Shared Memory:** A memory-mapped file named `Local\eci_shm_[handle]`.
* **Events:** Two Windows Events (`h_evt_ready` and `h_evt_processed`) act as a hardware-like handshake.
1. Host writes audio to Shared Memory.
2. Host signals `ready`.
3. Client reads audio.
4. Client signals `processed`.

---

### 🧩 Shared Memory Layout

When the client calls `set_buffer` (API ID 5), the host creates a memory-mapped file. The structure is strictly defined so both the Rust Host and your Client (e.g., Python) know exactly where every byte sits.

You must specify the samples of the audio buffer, e.g. 3300.

The memory is divided into a **Header** (first 12 bytes) and a **Data Buffer** (the rest).

| Offset (Bytes) | Field Name | Type | Description |
| --- | --- | --- | --- |
| **0 - 3** | `h_engine` | `u32` | **Input/Output:** In callback, the Engine Handle. **After processed**, the Client writes the return code here. |
| **4 - 7** | `msg` | `u32` | **Output:** The ECI Message ID (e.g., `eciWaveformBuffer`, `eciIndexReply`). |
| **8 - 11** | `lparam` | `i32` | **Output:** Message-specific data (e.g., number of samples or index value). |
| **12 - End** | `audio_data` | `i16[]` | **Output:** The raw PCM audio samples (16-bit Mono). |

---

### 🔄 The Synchronization Handshake

Because Shared Memory is "passive," we use two Windows Events to synchronize access. This prevents the Client from reading while the Host is still writing.

1. **Host writes** to the Shared Memory (Header + Audio).
2. **Host signals** `h_evt_ready` (The Client wakes up).
3. **Client reads** the `msg`, `lparam`, and `audio_data`.
4. **Client writes** a result code (usually `1` for Success) into the first 4 bytes (`h_engine`).
5. **Client signals** `h_evt_processed` (The Host wakes up).

---

### 📥 Response Bytes (Detailed Client View)

When the Client receives the `h_evt_ready` signal, it should interpret the bytes in the shared memory as follows:

#### 1. Identifying the Message (`msg` field at Offset 4)

The Client must check the `u32` at offset 4 to know what just happened in the engine:

* **`eciWaveformBuffer` (Value: 3):** New audio is available. Look at `lparam` (Offset 8) to see how many **samples** were written. The audio starts at Offset 12.
* **`eciIndexReply` (Value: 4):** A text index has been reached. `lparam` contains the index ID you inserted.
* **`eciPhonemeBuffer` (Value: 0):** Phoneme data is available (if enabled).

#### 2. Calculating Data Size

If the message is audio (`eciWaveformBuffer`), the total bytes to read from the buffer starting at offset 12 is:


$$\text{Total Bytes} = \text{lparam} \times 2$$


*(Since each sample is a 16-bit / 2-byte integer).*

#### 3. Returning the Result Code

The Host's `eci_callback` function is waiting for a return value to tell the legacy DLL what to do next. The client **must** write this value back to **Offset 0** before signaling `processed`:

* **`1` (eciDataProcessed):** Everything is fine, continue synthesis.
* **`2` (eciDataAbort):** Stop synthesis immediately.
* **`0` (eciDataNotProcessed):** The client ignored this specific message.

---





## 🛠 Extension Guide

### How to define new functions in the Lib Loader

If you need to support more functions from the legacy DLL:

1. Open `libLoader.rs`.
2. Add the function signature inside the `define_eci_api!` macro block.
3. Ensure the signature matches the C header (usually `stdcall` / `system`).

```rust
define_eci_api! {
    eciNewFunction(arg1: i32) -> i32;
}

```

### How to implement new APIs in the Worker

To expose a new feature to the IPC client:

1. Go to `worker.rs`.
2. Create a function and decorate it with the `#[api(ID)]` attribute.
3. Use the `RequestContext` to extract arguments.

```rust
#[api(20)] // New unique ID
fn my_new_feature(ctx: &RequestContext) -> Vec<u8> {
    let value = ctx.get_int(0);
    // Call eci()...
    pack_int(1)
}

```

---

## 🚀 The DLL & Launch Logic

The host is compiled as a dynamic library (`.dll`) to be launched via `rundll32.exe`.

### Execution Command

To launch the host correctly from Python or a Command Prompt, use the following syntax:

```bash
rundll32.exe "path\to\ibmtts_host32.dll",StartHost [PID]

```

### Required Parameter: `PID`

* **The Parent PID:** You must pass the Process ID (PID) of the client application (e.g., your Python script).
* **The Suicidal Thread:** The host starts a "Parent Monitor" thread. It checks every 5 seconds if the provided PID is still alive. If the parent process crashes or is closed, the host will automatically terminate to avoid leaving "zombie" processes in the system.

---

## 💻 Building a Client

To talk to this host, your client must:

1. **Connect:** Open `\\.\pipe\ibmtts_host32` as a file.
2. **Request Format:** Send a binary packet where:
* `Bytes 0-1`: Function ID (Little Endian `u16`).
* `Bytes 2+`: Parameters (4-byte integers or offsets to strings).
* after parameters: strings if there any, null terminated. The start offset of the strings must be set in the parameters.

See the file src\worker.rs to know about the order of the parameters for each endpoint.

3. **Read Response:**
* `Byte 0`: Type (0=Int, 1=UInt, 2=Bytes, 3=UTF8).
* `Remaining bytes`: The data response.



**Example Flow:**

1. Send ID `1` (LoadLibrary) with the path to `eci.dll`.
2. Send ID `3` (eciNew) to get an engine handle. Please store the handle.
3. Send ID `5` (set_buffer) to initialize shared memory audio streaming.
4. Set the events and open the shared memory block.


