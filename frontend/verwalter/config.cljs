(ns verwalter.config
    (:require
        [rum.core :as rum]
        ))

(defn key-value [[key, value]]
    [:.p key ": " value])

(defn role [[name, info]]
    [:.section
        [:.h3 "Role " name]
        [:.h4 "Renderers"]
        (map key-value (get info "renderers"))
        [:.h4 "Runtime"]
        (map key-value (get info "runtime"))
    ])

(defn render [cfg]
    [:.div
        [:.h1 "VERSION " (get cfg "verwalter_version")]
        [:.h2 "Machine Config"]
        (map key-value (get cfg "machine"))
        [:.h2 "Roles"]
        (map role (get cfg "roles"))
    ])

