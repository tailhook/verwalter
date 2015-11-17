(ns verwalter.config
    (:require
        [rum.core :as rum]
        ))

(defn key-value [[key, value]]
    [:.p key ": " value])

(defn role [[name, info]]
    [:.offset-left
        [:.h3 "Role " name]
        [:.h4 "Renderers"]
        (map key-value (get info "renderers"))
        [:.h4 "Runtime"]
        (map key-value (get info "runtime"))
    ])

(rum/defc render < rum/cursored rum/cursored-watch [cfg state]
    (let [show_machine (rum/cursor state [:show_machine])]
        [:.div
            [:.h1 "VERSION " (get cfg "verwalter_version")]
            (if @show_machine
                [:.accordion
                    {:on-click (fn [_] (do (reset! show_machine false)))}
                    [:.h2 "Machine Config"]
                    [:.offset-left
                        (map key-value (get cfg "machine"))]]
                [:.accordion__button
                    {:on-click (fn [_] (do (reset! show_machine true)))}
                    "Machine config"])
            [:.h2 "Roles"]
            (map role (get cfg "roles"))
        ]))

