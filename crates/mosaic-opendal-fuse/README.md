# Mosaic OpenDAL FUSE Adapter

A FUSE (Filesystem in Userspace) adapter for the [OpenDAL](https://opendal.apache.org/) data access library. This allows mounting an OpenDAL-supported storage backend, such as S3, as a local filesystem.

## Usage

To mount an S3 bucket, you first need to configure your credentials.

First, copy `.env.example` to `.env`:

```sh
cp .env.example .env
```

Then, fill in the required environment variables for your provider.

> [!NOTE]
> Depending on which service provider you're using, the environment variables that are required may differ. See the Rust [OpenDAL crate](https://docs.rs/opendal/latest/opendal/services/struct.S3.html#compatible-services) for more information on supported services and how to configure them.

> [!NOTE]
> If you are thinking on sharing the FUSE mount point with docker containers, you may need to tune host `/etc/fuse.conf` adding the `user_allow_other` option.

Once configured, you can run the following command to mount the filesystem:

```sh
cargo run --release -- --mount-path /path/to/mount
```

This will mount the S3 bucket at `/path/to/mount`.

### Command-line arguments

- `--mount-path <PATH>`: The path to mount the FUSE filesystem at. If not specified, a temporary directory is used instead.
- `--socket <PATH>`: The path to listen on for socket connections. Defaults to `/tmp/mosaic_opendal_fuse.sock`.

## Development

To build the project, you can use the standard Cargo commands:

```sh
cargo build
```

To run the tests:

```sh
cargo test
```

### Tracing

The project uses the [tracing](https://crates.io/crates/tracing) crate for logging. To enable logging, set the `RUST_LOG` environment variable to the desired log level. By default, the log level is set to `info`.

For example, to enable debug logging:

```sh
RUST_LOG=debug cargo run --release -- --mount-path /path/to/mount
```
