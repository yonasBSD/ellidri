[![builds.sr.ht status](https://builds.sr.ht/~taiite/ellidri.svg)](https://builds.sr.ht/~taiite/ellidri?)
[![crates.io](https://img.shields.io/crates/v/ellidri.svg)](https://crates.io/crates/ellidri)

# kawaii

ellidri, your kawaii IRC server.

Join the IRC channel: [#ellidri on freenode][irc]!

[irc]: https://webchat.freenode.net/#ellidri


## Features

- RFC [1459][0] and [2812][1] compliance (almost! see [#1][2])
- TLS support
- Multiple listening ports
- Capabilities (version 302)
- kawaii messages

Supported capabilities:

- [cap-notify](https://ircv3.net/specs/core/capability-negotiation#cap-notify)
- [echo-message](https://ircv3.net/specs/extensions/echo-message-3.2)
- [message-ids](https://ircv3.net/specs/extensions/message-ids)
- [message-tags](https://ircv3.net/specs/extensions/message-tags)
- [server-time](https://ircv3.net/specs/extensions/server-time-3.2.html)

ellidri only supports the UTF-8 encoding for messages, though for now it only
supports ASCII casemapping for channels.

[0]: https://tools.ietf.org/html/rfc1459
[1]: https://tools.ietf.org/html/rfc2812
[2]: https://todo.sr.ht/~taiite/ellidri/1


## Build and install

Prerequisites:

- The Rust compiler (at least version 1.39) and Cargo: <https://rustup.rs/>
- On Linux, the OpenSSL library and its development files

Install ellidri with `cargo install ellidri`

Build it with `cargo build`.  Append the `--release` flag to build with
optimizations enabled.


## Usage

ellidri needs a configuration file to run.  Its format is the following:

```
file   =  *( line "\n" )
line   =  sp key sp value sp
key    =  word
value  =  *( word / sp )
sp     =  any sequence of whitespace
```

An example configuration file with all settings and their defaults can be found
in `doc/ellidri.conf`.

To start ellidri, pass the path of the configuration file as its first argument:

```shell
cargo run -- doc/ellidri.conf
# or
./target/debug/ellidri doc/ellidri.conf
# or
./target/release/ellidri doc/ellidri.conf
```


## Contributing

Patches are welcome!  Here are some links to get started:

- Documentation: <https://docs.rs/ellidri>
- Git repository: <https://git.sr.ht/~taiite/ellidri>
- Send patches to the mailing list: <https://lists.sr.ht/~taiite/public-inbox>
- Report bugs on the issue tracker: <https://todo.sr.ht/~taiite/ellidri>


## Acknowledgments

ellidri couldn't have existed without the help of <https://ircdocs.horse>.
Thank you Daniel Oaks and [all other contributors][ac]!

Also thanks to the [IRCv3 working group][i3] for all the work on modernizing
the IRC protocol!

[ac]: https://github.com/ircdocs/modern-irc/graphs/contributors
[i3]: https://ircv3.net/charter


## License

ellidri is under the ISC license.  See `LICENSE` for a copy.
