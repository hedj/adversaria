:- encoding(utf8).
/** <module> The memex's parser of controlled English, as a server

Run as a child process of the memex: swipl prolog/ace_server.pl
The query protocol server (server.pl) with APE loaded (ace.pl), in a
process of its own: loading APE and Clex takes about a second, which the
reasoning engines need not pay.
*/

%   Loaded before the server, so the server counts its predicates as its
%   own (server.pl, server_predicate/1).
:- use_module(ace).
:- consult(server).
