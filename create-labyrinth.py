#!/usr/bin/env python3

from pymongo import MongoClient
from pprint import pprint
import sys
import os
import json
import re
from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build
from googleapiclient.errors import HttpError

SCOPES = ['https://www.googleapis.com/auth/spreadsheets.readonly']
SPREADSHEET_ID = '1NS8yGoFvIOdjbdJIxCXG0rSPESTSyyjoVufbtWJBZTg'
SPREADSHEET_RANGE = 'Game1!A1:O15'
RE_UINT = re.compile(r'\d+')
ENTRY = "Entry"
EXIT = "Exit"
X = "X"

db = None

def generate(maze):
    game_id = db.games.insert_one({ "number": 0x1ee7cafe}, None).inserted_id
    grid = maze["grid"]
    width = maze["width"]
    height = maze["height"]
    rooms = []
    cursor = db.riddles.find()
    riddles = {}
    for riddle in cursor:
        riddles[riddle["level"]] = riddle["_id"]
    pprint(riddles)
    for y in range(height):
        for x in range(width):
            c = grid[x][y]
            if c in [X, ENTRY, EXIT]:
                neighbors = []
                level = None
                # to north
                if y > 0:
                    level = grid[x][y - 1]
                    if RE_UINT.match(level):
                        level = int(level)
                        neighbors.append({
                            "direction": "n",
                            "riddle_id": riddles[level],
                        })
                # to east
                if x < width - 1:
                    level = grid[x + 1][y]
                    if RE_UINT.match(level):
                        level = int(level)
                        neighbors.append({
                            "direction": "e",
                            "riddle_id": riddles[level],
                            "level": level,
                        })
                # to south
                if y < height - 1:
                    level = grid[x][y + 1]
                    if RE_UINT.match(level):
                        level = int(level)
                        neighbors.append({
                            "direction": "s",
                            "riddle_id": riddles[level],
                            "level": level,
                        })
                # to west
                if x > 0:
                    level = grid[x - 1][y]
                    if RE_UINT.match(level):
                        level = int(level)
                        neighbors.append({
                            "direction": "w",
                            "riddle_id": riddles[level],
                            "level": level,
                        })
                room = {
                    "neighbors": neighbors,
                    "game_id": game_id,
                }
                if c == ENTRY:
                    room.update({ "entry": True })
                if c == EXIT:
                    room.update({ "exit": True })
                rooms.append(room)
    pprint(rooms)
    result = db.rooms.insert_many(rooms)


def find_entry(maze):
    for x in range(maze["width"]):
        for y in range(maze["height"]):
            if maze["grid"][x][y] == maze["entry"]:
                return (x, y)
    return None


def main():
    global db
    creds = None

    if os.path.exists('token.json'):
        creds = Credentials.from_authorized_user_file('token.json', SCOPES)
    if not creds or not creds.valid:
        if creds and creds.expired and creds.refresh_token:
            creds.refresh(Request())
        else:
            flow = InstalledAppFlow.from_client_secrets_file(
                'credentials.json', SCOPES)
            creds = flow.run_local_server(port=0)
        # Save the credentials for the next run
        with open('token.json', 'w') as token:
            token.write(creds.to_json())

    print("Loading spreadsheet ...")
    maze = None
    try:
        service = build('sheets', 'v4', credentials=creds)
        sheet = service.spreadsheets()
        result = sheet.values().get(spreadsheetId=SPREADSHEET_ID,
                                    range=SPREADSHEET_RANGE).execute()
        values = result.get('values', [])
        values = [*zip(*values)]
        maze = {
            "width": len(values),
            "height": len(values[0]),
            "grid": values,
        }
    except HttpError as err:
        print(err)


    print("Connecting to MongoDB ...")
    client = MongoClient(
        "mongodb://127.0.0.1:27017/?readPreference=primary&serverSelectionTimeoutMS=2000&appname=mongosh%201.2.3&directConnection=true&ssl=false"
    )
    db = client.labyrinth
    db.games.drop()
    db.rooms.drop()
    db.directions.drop()
    # transpose maze
    generate(maze)


if __name__ == "__main__":
    main()
