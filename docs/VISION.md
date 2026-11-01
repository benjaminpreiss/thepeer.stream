# Browser access to a localhost web app

A person shares one explicitly selected localhost web app from a computer.
A matching desktop CLI or Android app pairs with a one-shot offer and opens the
app in the browsing device's normal browser through a local HTTP gateway.
The sharing CLI owns the target; the browsing client owns its encrypted Saorsa
connection and browser gateway. The user controls the session with Stop.

The route policy tries eligible Direct candidates, followed by an explicitly
configured single-carrier MASQUE Relay. Authentication checks the sharing
computer's key on the retained connection before sending the grant. The browser
gateway applies bounded HTTP policy and binds to loopback.

See [current features](../README.md#features), [CLI pairing](CLI.md#pairing-and-ownership)
and [validation history](../CHANGELOG.md).
