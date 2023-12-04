# netproxy

It's a fast and convenient network proxy.

Route data according to some configuration sentences.


## Usage

### Start up

no specify socket : 'netproxy'

specify socket : 'netproxy -s 127.0.0.1:0'

* For more information

'netproxy --help'

### Configuration

* send configuration to socket, an example of TCP: 'tcp-tcp 127.0.0.1:10000 127.0.0.1:20000'

* some route with proportion: 'tcp-tcp 127.0.0.1:10000 127.0.0.1:20000,127.0.0.1:20001 1:1'

first str is accept protocol and route target protocol, split by '-', if only one,the other is the same.

second str is socket addr to accept data.

third str is target socket addr, one or several, where data transfer to, split by ','

fourth str is proportion of data transfer to target, split by ':'. it's digit and correspondence with third. if it's not digit, replace with 0. if number is less than socket addrs, fill with 1. it can be omitted.

usable protocol include "tcp","tls","udp","http","http_pt".

* set certificate: 

'certificate f ./ ***'

second str "f" means to get from file.

third str is file path.

fourth str is certificate password.

'certificate s 127.0.0.1:10000 ***'

second str "s" means to get from socket.

* show proxy state:

'state'

show state of all proxy servers.

'state 127.0.0.1:10000'

specify socket to show state.

* shutdown service:

'shutdown 127.0.0.1:10000'

specify socket to shutdown a server.
