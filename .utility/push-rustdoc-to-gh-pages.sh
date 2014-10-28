if [ "$TRAVIS_REPO_SLUG" == "GGist/RustBT" ] && [ "TRAVIS_PULL_REQUEST" == "false" ] && [ "$TRAVIS_BRANCH" == "master" ]; then

    echo -e "Publishing rustdoc...\n"
    
    cargo doc --no-deps
    
    cp target/doc $HOME/rustdoc-latest
    
    cd $HOME
    git config --global user.email "travis@travis-ci.org"
    git config --global user.name "travis-ci"
    git clone --quiet --branch=gh-pages https://${GH_TOKEN}@github.com/GGist/RustBT gh-pages > /dev/null
    
    cd gh-pages
    cp -Rf &HOME/rustdoc-latest/* .
    git add -f .
    git commit -m "Latest rustdoc on successful travis build $TRAVIS_BUILD_NUMBER auto-pushed to gh-pages"
    git push -fq origin gh-pages > /dev/null
    
    echo -e "Published rustdoc to gh-pages.\n"

fi