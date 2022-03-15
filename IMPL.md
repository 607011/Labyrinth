# Notizen zur Implementierung

Router: https://blog.logrocket.com/a-minimal-web-service-in-rust-using-hyper/
JWT: https://crates.io/crates/jwt | https://jwt.io/

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
     "pin": u32,
     "activated": bool,
   }
   ```

- Struktur des Datenbankeintrags für einen Raum:
   ```json
   {
     "_id": <ObjectID>,
     "doors": [
         {
             id: <ObjectID>,
             direction: "one of N, E, S, W",
         }
     ],
   }
   ```

- Struktur des Datenbankeintrags für eine Tür:
   ```json
   {
     "_id": <ObjectID>,
     "task": "description of the problem to solve. can be empty",
     "data": "https://escape.quiz/files/4cee645e-5a21-4a76-b7c2-061d122c93bf.zip", // URL to a zip or 7z archive containing necessary files to solve the problem
     "solution": "the solution of the problem",
   }
   ```