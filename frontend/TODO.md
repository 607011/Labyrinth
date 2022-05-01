# To-do

* [ ] `disable 2fa` implementieren
* [ ] falls schon TOTP aktiviert wurde, bei `enable 2fa totp` fragen, ob das wirklich gewünscht ist
* [X] `login` prüfen
  * [X] `login` plus TOTP checken
  * [X] `login` plus FIDO2 checken
* [X] `enable 2fa` checken
  * [X] `enable 2fa totp` checken
  * [X] `enable 2fa fido2` checken
* [X] Checken, ob Auswahl zwischen TOTP und FIDO2 bei `login` funktioniert
* [X] `register` prüfen
* [ ] Sicherstellen, dass alleiniges Einloggen mit zweitem Faktor, insbesondere FIDO2, *nicht* funktioniert
* [ ] `passwd` zum Ändern des Passworts implementieren
* [ ] Einloggen mit Wiederherstellungsschlüssel implementieren -> danach Ändern des Passworts mit `passwd` erzwingen
* [ ] `whoami` soll Infos über konfigurierte zweite Faktoren ausgeben
* [ ] Aufzeichnen, wie lange man von der Anzeige eines Rätsels bis zur Lösung gebraucht hat
* [ ] `highscores` implementieren
  * [ ] Liste mit absoluten Scores
  * [ ] Liste mit Scores pro Zeit
    * [ ] gerechnet vom Zeitpunkt der Registrierung
    * [ ] gerechnet vom Zeitpunkt der Anzeige eines Rätsels bis zur Eingabe der richtigen Lösung
