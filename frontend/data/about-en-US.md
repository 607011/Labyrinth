# About this website

This is an escape game. Find your way through the labyrinth to the exit. The labyrinth is constructed from rooms which are interconnected by doors. To step through a door, it has to been opened first by typing in a password. The password is the solution of a riddle you have to solve. Once opened, a door stays open. All your data is stored on the server, so that your game session is restored each time you login. That means, you can play the game with the same state in different browsers.

Good luck!

## Background

This software consists of two parts: the [frontend](https://github.com/ola-ct/Labyrinth-Frontend) (what you’re currently seeing) and the [backend](https://github.com/ola-ct/Labyrinth) (a webservice giving access to the game data). The webservice has a [REST](https://en.wikipedia.org/wiki/Representational_state_transfer) [API](https://en.wikipedia.org/wiki/API) implemented in [Rust](https://rust-lang.org/). It’s connected to a [MongoDB](https://mongodb.com/) database, which stores the information about the rooms, the doors, the riddles, and you, the registered user. Users are authenticated by JSON Web Tokens ([JWT](https://jwt.io/)). 

*Labyrinth* is a private project by me, [Oliver Lau](mailto:oliver@ersatzworld.net).

## Contact

If you encounter problems with this software, please report them via the GitHub project pages for the [frontend](https://github.com/ola-ct/Labyrinth-Frontend) (what you’re currently seeing) and the [backend](https://github.com/ola-ct/Labyrinth). Feel free to leave encouraging and/or critical comments, or propose new features. If you've got a nice fresh idea for a new riddle, I'd be thankful if you contact me via [e-mail]((mailto:oliver@ersatzworld.net)).

## License

Copyright &copy; 2022 Oliver Lau &lt;oliver@ersatzworld.net&gt;

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE. 