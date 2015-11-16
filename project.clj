(defproject verwalter "0.1.0-SNAPSHOT"
  :dependencies [[org.clojure/clojure "1.7.0"]
                 [org.clojure/clojurescript "1.7.122"]
                 [org.clojure/core.async "0.1.346.0-17112a-alpha"]
                 [cljs-ajax "0.3.11"]
                 [rum "0.5.0"]]

  :plugins [[lein-cljsbuild "1.1.0"]
            [lein-figwheel "0.4.1"]]

  :source-paths ["frontend"]

  :clean-targets ^{:protect false} ["public/js"]

  :cljsbuild {
    :builds [{:id "dev"
              :source-paths ["frontend"]

              :figwheel { :on-jsload "verwalter.core/on-js-reload" }

              :compiler {:main verwalter.core
                         :asset-path "/js/deps"
                         :output-to "public/js/main.js"
                         :output-dir "public/js/deps"
                         :source-map-timestamp true }}
             {:id "min"
              :source-paths ["frontend"]
              :compiler {:output-to "public/js/main.min.js"
                         :main verwalter.core
                         :optimizations :advanced
                         :pretty-print false}}]}

  :figwheel {
             :http-server-root "public" ;; default and assumes "resources"
             :server-port 8378 ;; default
             ;; :server-ip "127.0.0.1"

             :css-dirs ["public/css"] ;; watch and update CSS

             ;; Start an nREPL server into the running figwheel process
             ;; :nrepl-port 7888

             ;; Server Ring Handler (optional)
             ;; if you want to embed a ring handler into the figwheel http-kit
             ;; server, this is for simple ring servers, if this
             ;; doesn't work for you just run your own server :)
             ;; :ring-handler hello_world.server/handler

             ;; To be able to open files in your editor from the heads up display
             ;; you will need to put a script on your path.
             ;; that script will have to take a file path and a line number
             ;; ie. in  ~/bin/myfile-opener
             ;; #! /bin/sh
             ;; emacsclient -n +$2 $1
             ;;
             ;; :open-file-command "myfile-opener"

             ;; if you want to disable the REPL
             ;; :repl false

             ;; to configure a different figwheel logfile path
             ;; :server-logfile "tmp/logs/figwheel-logfile.log"
             })
