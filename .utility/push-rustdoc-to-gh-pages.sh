#!/bin/bash

echo -e $TRAVIS_REPO_SLUG
echo -e $TRAVIS_PULL_REQUEST
echo -e $TRAVIS_BRANCH


    echo -e "Publishing rustdoc...\n"
    
    mkdir $HOME/rustdoc-latest
    cargo doc --no-deps
    cp -r target/doc/* $HOME/rustdoc-latest/.
    
    cd $HOME
    git config --global user.email "travis@travis-ci.org"
    git config --global user.name "travis-ci"
    git clone --quiet --branch=gh-pages https://${GH_TOKEN}@github.com/GGist/RustBT gh-pages > /dev/null
    
    cd gh-pages
    cp -rf $HOME/rustdoc-latest/* .
    git add -A
    git commit -m "Latest rustdoc on successful travis build $TRAVIS_BUILD_NUMBER auto-pushed to gh-pages"
    git push -fq origin gh-pages > /dev/null
    
    echo -e "Published rustdoc to gh-pages.\n"
