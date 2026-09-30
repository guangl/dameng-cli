function __dm_complete
    set -l words (commandline -xpc)
    set -e words[1]
    set -l current (commandline -ct)
    if test -z "$current"
        command dm complete --empty-word -- $words 2>/dev/null
    else
        command dm complete -- $words "$current" 2>/dev/null
    end
end
complete -c dm -f -a '(__dm_complete)'
