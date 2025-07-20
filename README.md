# Website SSH

Browse my website via SSH by

```sh
ssh viddrobnic.com
```

This is basically just a modified version of [simple-rss](https://github.com/viddrobnic/simple-rss) (my simple RSS reader).
It's modified to serve just my website's RSS feed as an SSH server using [russh](https://github.com/Eugeny/russh).

You can run it yourself (even though it probably doesn't make a lot of sense) with:

```sh
cargo run -- -p <port_number>
ssh -p <port_number> localhost
```

## License

The project is licensed under the [MIT License](LICENSE).
