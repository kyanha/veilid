#!/bin/bash

# ensure adb is installed
if command -v docker &> /dev/null; then 
    echo '[X] docker is available in the path'
else
    echo 'docker is not available in the path'
    exit 1
fi

# pull jaeger
echo pulling Jaeger
docker pull jaegertracing/all-in-one:1.35

# run jaeger
echo running Jaeger
docker run -d --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:1.35 --collector.otlp.enabled=true $@

read -p "Press [Enter] key to stop jaeger"

docker stop jaeger
docker rm jaeger
