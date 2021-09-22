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
# The gRPC server configuration.
[grpc]
# The address to bind on.
ip = "127.0.0.1"
# The port to bind on.
port = 8009

# The database engine configuration.
[engine]
# The time (in minutes) to wait between flushes to disk.
cache_time = 1
# The database directory to use. Will be created if it does not exist.
dir = "rusdb"
```