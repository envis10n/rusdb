# rusdb

A rusty NoSQL database server and storage engine.

## A few notes

This is NOT production ready, and likely will never be. I'm exploring writing a database, and solving the problems that come up.

#### **!!! THERE IS NO AUTHENTICATION !!!**

*I plan to add something like this in the future, but for the time being, there is nothing there.*

The last big note: *At the time of writing this, there is no mechanism for freeing under-utilized cached collections.*

## Usage

This crate provides the actual server binary, and an implementation of the gRPC protos used to communicate between client and server.

`cargo build` should work fine, and the resulting binary will be located in the target build directory.

## Configuration

A configuration file will be looked for in the CWD of where the binary is run from, named `.rusdb.toml`.

If a configuration file does not exist, and the file can be created, a default file will be written to disk.

Example:
```toml
[grpc]
ip = "127.0.0.1" # Required - gRPC bind hostname/address.
port = 8009 # Required - gRPC bind port

[engine]
cache_time = 1 # Required - Cache disk sync time in minutes.
flush_time = 10 # Required - Flush time in minutes.
dir = "./rusdb" # Optional - Default "./rusdb"

[logging] # Optional - Default: None
path = "./rusdb.log" # Optional - Default: "./rusdb.log" - Relative paths place it inside of the data directory.
level = 4 # Optional - Default: 2 (LevelFilter::Info) - Anything outside of 0-5 will be LevelFilter::Trace.

```