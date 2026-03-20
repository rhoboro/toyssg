#!/bin/bash

cargo run
rsync -a --delete dist/ docs/
git add .
git commit -m "deploy"
git push
