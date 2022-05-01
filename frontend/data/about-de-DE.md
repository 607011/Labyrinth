# Über diese Website

Dies ist ein [Escape Game](https://de.wikipedia.org/wiki/Escape_Game). Finde den Weg nach draußen durch ein Labyrinth. Das Labyrinth besteht aus Räumen, die durch Türen miteinander verbunden sind. Um durch eine Tür gehen zu können, musst du sie zunächst durch Eingabe eines Passworts öffnen. Das Passwort ergibt sich aus einem Rätsel.

Mit jedem gelösten Rätsel verbesserst du deinen Score. Versuch, so viele Punkte wie möglich zu erzielen, indem du kein Rätsel auslässt. Obacht, bei einigen Rätseln bekommst du Punkte abgezogen, wenn du eine falsche Antwort gibst.

Einmal geöffnet, bleibt eine Tür offen. Sämtliche Spieldaten werden auf einem Server gespeichert. Das heißt, du kannst eine Spielsitzung nach einem Login fortsetzen, auch mit einem anderen Browser und sogar auf einem anderen Rechner.

Viel Spaß und viel Erfolg!

## Hintergründiges

Diese Software besteht aus zwei Teilen: dem [Frontend](https://github.com/ola-ct/Labyrinth-Frontend) (das, was du gerade siehst) und dem [Backend](https://github.com/ola-ct/Labyrinth) (ein Webservice, der Zugriff auf die Spieldaten bietet). Der Webservice hat ein [REST](https://en.wikipedia.org/wiki/Representational_state_transfer)-[API](https://en.wikipedia.org/wiki/API), das in [Rust](https://rust-lang.org/) geschrieben wurde. Es kommuniziert mit einer [MongoDB](https://mongodb.com/)-Datenbank, die Informationen über die Räume, die Türen, die Rätsel und die User speichert. Benutzer werden über JSON Web Tokens ([JWT](https://jwt.io/)) authentifiziert.

*Labyrinth* ist ein privates Projekt von [Oliver Lau](mailto:oliver@ersatzworld.net).

## Kontakt

Falls du Bugs in dieser Software entdeckst, melde sie bitte über die jeweiligen GitHub-Projektseiten für das [Frontend](https://github.com/ola-ct/Labyrinth-Frontend) und das [Backend](https://github.com/ola-ct/Labyrinth). Aufmunternde und/oder kritische Kommentare sowie Vorschläge für neue Features sind ebenfalls willkommen. Falls du eine coole Rätselidee hast, halte damit bitte nicht hinter dem Berg, sondern schick sie mir per [E-Mail](mailto:oliver@ersatzworld.net).
