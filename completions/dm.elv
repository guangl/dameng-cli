set edit:completion:arg-completer[dm] = {|@words|
    dm complete -- $@words[1..] 2>/dev/null
}
