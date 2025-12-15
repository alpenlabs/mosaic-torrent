# Mosaic OpenDAL FUSE Adapter

A FUSE (Filesystem in Userspace) adapter for the [OpenDAL](https://opendal.apache.org/) data access library. This allows mounting an OpenDAL-supported storage backend, such as S3, as a local filesystem.

## Usage

To mount an S3 bucket, you first need to configure your credentials. You can do this by setting the necessary environment variables or using the canonical configuration files for your provider. See the Rust [OpenDAL crate](https://docs.rs/opendal/latest/opendal/services/struct.S3.html#compatible-services) for more information on supported services and how to configure them.

Then, you can run the following command to mount the filesystem:

```sh
cargo run --release --mount-path /path/to/mount
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
