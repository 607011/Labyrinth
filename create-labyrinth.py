#!/usr/bin/env python3

from pymongo import MongoClient
from pprint import pprint
import sys
import os
import json
import re
from itertools import zip_longest
from dotenv import load_dotenv
from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build
from googleapiclient.errors import HttpError

SCOPES = ['https://www.googleapis.com/auth/spreadsheets.readonly']
SPREADSHEET_ID = '1NS8yGoFvIOdjbdJIxCXG0rSPESTSyyjoVufbtWJBZTg'
SPREADSHEET_RANGE = 'Game1!A1:O15'
RE_UINT = re.compile(r'\d+')
ENTRY = 'Entry'
EXIT = 'Exit'
X = 'X'

db = None


def generate(maze, game_id):
    grid = maze['grid']
    width = maze['width']
    height = maze['height']
    rooms = []
    riddles_cursor = db.riddles.find().sort('level', 1)
    riddles = {}
    for riddle in riddles_cursor:
        riddles[riddle['level']] = riddle['_id']
    pprint(riddles)

    def cond_append(neighbors, level, direction):
        if type(level) == int:
            riddle_id = riddles.get(level)
            assert(riddle_id is not None)
            neighbors.append({
                'direction': direction,
                'riddle_id': riddles[level],
                'level': level,
            })

    room_number = 1
    for y in range(height):
        for x in range(width):
            c = grid[x][y]
            print(x,y,c)
            if c in [X, ENTRY, EXIT]:
                neighbors = []
                # to north
                if y > 0:
                    cond_append(neighbors, grid[x][y - 1], 'n')
                # to east
                if x < width - 1:
                    cond_append(neighbors, grid[x + 1][y], 'e')
                # to south
                if y < height - 1:
                    cond_append(neighbors, grid[x][y + 1], 's')
                # to west
                if x > 0:
                    cond_append(neighbors, grid[x - 1][y], 'w')
                if len(neighbors) > 0:
                    pprint(neighbors)
                room = {
                    'neighbors': neighbors,
                    'game_id': game_id,
                    'number': room_number,
                }
                if c == ENTRY:
                    room.update({ 'entry': True })
                if c == EXIT:
                    room.update({ 'exit': True })
                rooms.append(room)
                room_number += 1
    pprint(rooms)
    result = db.rooms.insert_many(rooms)


def main():
    global db
    creds = None
    load_dotenv()
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

    print('Loading spreadsheet ...')
    maze = None
    try:
        service = build('sheets', 'v4', credentials=creds)
        sheet = service.spreadsheets()
        result = sheet.values().get(spreadsheetId=SPREADSHEET_ID,
                                    range=SPREADSHEET_RANGE).execute()
        values = result.get('values', [])
        major_dim = result.get('majorDimension', 'ROWS')
        assert(major_dim == 'ROWS')
        longest_row_len = max([len(row) for row in values])
        print(f'longest_row_len = {longest_row_len}')
        grid = []
        for row in values:
            row = list(map(lambda v: int(v) if v.isnumeric() else None if v == '' else v, row))
            if len(row) < longest_row_len:
                row.extend([None]*(longest_row_len - len(row)))
            grid.append(row)
        grid = list(zip_longest(*grid))
        maze = {
            'width': len(grid),
            'height': len(grid[0]),
            'grid': grid,
        }
    except HttpError as err:
        print(err)


    print('Connecting to MongoDB ...')
    client = MongoClient(f'{os.environ["DB_URL"]}/?readPreference=primary&directConnection=true&ssl=false')
    db = client.labyrinth
    db.games.drop()
    db.rooms.drop()
    game_id = db.games.insert_one({
        'name': 'My very first labyrinth ;-)',
        'number': 0,
        'maze': maze,
    }).inserted_id
    print(f'Created game with ID {game_id}')
    generate(maze, game_id)


if __name__ == '__main__':
    main()
