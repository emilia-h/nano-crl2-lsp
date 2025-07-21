
# nano-crl2-lsp

A language server protocol (LSP) implementation of the [mCRL2](https://mcrl2.org) model checking language.

The client implementation and other JavaScript boilerplatse in this repository is based on the
[tower-lsp boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate) repository. The license for that can
be found in [`./LICENSE`](./LICENSE).

The code in the [`./server`](./server/README.md) directory is my own and is separate from the client implementation. As
such, I own its copyright and it has its own license.

## Technical Details

I use my own [nanoCRL2](https://github.com/emilia-h/nano-crl2) library for this extension instead of the original
[mCRL2](https://github.com/mCRL2org/mCRL2) implementation. This is because that implementation is not really made to be
used by a language server. On the other hand, nanoCRL2 is a more lightweight, query-based compiler with no global
state, which has the advantage that editor features can call arbitrary semantic analysis passes and only pay for the
parts that are strictly necessary. For instance, if you want to ctrl-click on a symbol to "go to definition", the
compiler only runs the name lookup pass for that symbol (i.e., it does not run the entire compiler pipeline to find the
location of the definition of one symbol).

## Building and Running

Run `npm i` and then `npm run build` in the root directory. You can then press F5 in VSCode to test this extension
locally.

## Publishing

Run `npm i` and then bundle appropriately by using `npm run build` (which uses esbuild). Also, run
`cargo build --release` and copy the `nano_crl2_lsp` executable into the `server/` folder.

For publishing the VSCode extension, refer to
[Publishing Extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension).

For publishing the VSCodium extension, refer to
[Publishing Extensions](https://github.com/eclipse/openvsx/wiki/Publishing-Extensions). I publish to namespace
`nano-crl`. I run `ovsx publish -p <token>` (optionally with flag `--pre-release`), which should also run
`npm run build` automatically.
