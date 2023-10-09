# netproxy

It's a fast and convenient network proxy.
Route data according to some configuration sentences.


## Usage

* Start up

no specify socket : 'netproxy'

specify socket : 'netproxy -s 127.0.0.1:0'

* Configuration

send configuration to socket, an example of TCP:
'tcp 127.0.0.1:10000 127.0.0.1:20000'

some route with proportion:
'tcp 127.0.0.1:10000 127.0.0.1:20000,127.0.0.1:20001 1:1'

* For more information

'netproxy --help'
