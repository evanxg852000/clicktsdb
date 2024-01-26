#!/usr/bin/python3

import uuid
import time
import random
from random import randint
import requests

current_time = int(time.time() * 1000) # convert in milliseconds
locations = ["pittsburgh", "tokyo", "milan", "nantes", "frankfurt", "capetown"]
devices = {}
for location in locations:
    for i in range(randint(40,80)):
        if location not in devices:
            devices[location] = []
        devices[location].append(str(uuid.uuid4()))

def generate_batch(num_batch):
    data = ""
    for i in range(num_batch):
        loc = random.choice(locations)
        id = random.choice(devices[location])
        min = randint(0, 155000)
        max = randint(min, 200000)
        avg = min + max / 2
        timestamp =  current_time - randint(0, 3000)
        
        line = f"counter,location={loc},device_id={id} minimum={min},maximum={max},average={avg} {timestamp}"
        data += line + '\n'
    return data

def main():
    i = 1
    while True:
        payload = generate_batch(1000)
        # print(payload, "\n")
        resp = requests.post('http://localhost:3000/influxdb', data = payload)
        print(f"Batch Num: `{i}`, Status: `{resp.status_code}`")
        time.sleep(3)
        i += 1

if __name__ == "__main__":
    main()












