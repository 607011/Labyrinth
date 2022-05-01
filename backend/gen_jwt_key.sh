#!/bin/sh
cat /dev/urandom | head -c32 > JWT_SECRET_KEY
