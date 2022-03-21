# Notizen zur Implementierung

- Rate-Limiter für API-Requests implementieren, damit niemand Passwörter durch Ausprobieren herausfinden kann.


- Struktur des Datenbankeintrags für ein Spiel:
   ```json
   {
     "_id": <ObjectID>,
   }
   ```

- Struktur des Datenbankeintrags für einen Benutzer:
   ```json
   {
     "_id": <ObjectID>,
     "username": "<unique username>",
     "role": "['User', 'Admin'],
     "password": {
       "salt": "<short randomly generated string>",
       "hash": <ByteArray>
     },
     "pin": Int64,
     "activated": bool,
     "last_login": Date,
     "registration_started": Date,
     "registered": Date,
     "in_room": <ObjectId>,
     "solved": [
       <ObjectId>,
       ...
     ],
   }
   ```

- Struktur des Datenbankeintrags für ein Labyrinth:
   ```json
   {
     "_id": <ObjectID>,
     "number": Int64
   }
   ```

- Struktur des Datenbankeintrags für eine Richtung:
   ```json
   {
     "_id": <ObjectID>,
     "direction": String,
     "riddle_id": <ObjectID>,
   }
   ```

- Struktur des Datenbankeintrags für einen Raum:
   ```json
   {
     "_id": <ObjectID>,
     "directions": [ <Direction> ],
     "labyrinth_od": <ObjectID>
   }
   ```

- Struktur des Datenbankeintrags für eine Rätsel:
   ```json
   {
     "_id": <ObjectID>,
     "task": "description of the problem to solve. can be empty",
     "level": Int32,
     "data": "https://escape.quiz/files/4cee645e-5a21-4a76-b7c2-061d122c93bf.zip", // URL to a zip or 7z archive containing necessary files to solve the problem
     "solution": "the solution of the problem",
   }
   ```