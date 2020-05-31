SELECT pos, definition
FROM words w
INNER JOIN definitions d
ON w.id = d.word_id
WHERE w.word = :word
ORDER BY pos, definition;