# osbuild-ks

Transform kickstart files into osbuild manifest files. Just another rainy
sunday project.

## Usage
You can run `osbuild-ks` with `osbuild-ks <src> <dst>`. The `<src>` has to be a
file in the Kickstart format, the resulting osbuild manifest will be written to
`<dst>`. If your Kickstart file includes other files then you will want to pass
`-I <include>` for the path to use for the other files if they aren't in `.`.

```
€ ./target/debug/osbuild-ks --help
osbuild-ks 0.1.0
Simon de Vlieger <cmdr@supakeen.com>
Convert Kickstart files to osbuild manifests.

USAGE:
    osbuild-ks [OPTIONS] <src> <dst>

ARGS:
    <src>    Kickstart input file
    <dst>    osbuild manifest output file

OPTIONS:
    -h, --help                 Print help information
    -I, --include <include>    include path for kickstart files [default: .]
    -V, --version              Print version information
```
