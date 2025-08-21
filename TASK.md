# Backend Engineering Technical Interview

Feel free to use AI, our goal is to maximize our insight into your skills while minimizing the cost of time and effort on our candidates, the interview will be a discussion around the solutions of the 2 challenges

Both challenges below are take home ones, once you are ready feel free to schedule an interview.
We expect both challenges to take you less than 4 hours of your time.

# **First: Coding Challenge ( Rust or Go )**

Build an in-memory **limit order book** with a single entry point: `place_order`. The function must **attempt to match** the incoming order against the opposite side and then **add any remaining quantity** to the book. Example:An incoming sell order should be attempted to match/trade with the best existing buy orders on the book, a best buy order would be the one with the highest price (willing to pay the most ), a best selling order on the sell side is the one that is willing to sell for the least price:

- **Price–time priority.** Fill the **oldest** orders at the **best** price level first.
- **Trade price** = **resting (book) order’s price**.
- **Partial fills** allowed. Remove fully filled resting orders; decrease quantity otherwise.
- **Remainder**: if the incoming order still has quantity, **add it to the book** at its limit price with **newest** time priority.
- **No cancels, no expiries, no self-match prevention** (keep it simple).

### **API (you may adjust exact names/types)**

Implement in **Rust** or **Go**:

- `place_order(side, price, quantity, id) -> trades`
    - **side**: `Buy` or `Sell`
    - **price**: integer
    - quantity: integer
    - **returns**: list of trades `(price, qty, maker_id, taker_id)` in match order
- `best_buy() -> Option<(price, tatal_quantity)>` total_quantity being the sum of all orders quantities at that price .
- `best_sell() -> Option<(price, tatal_quantity)>`

Points are about correctness, design and convention ( Idiomatic Rust or Go ) and performance.

# **Technical discussion on a previous project**

In any of your previous experiences pick a project that you are proud of and that stands out for you as on that was very difficult and challenging which you owned and completed with success. Present us with a small Architecture diagram and description of the problem and how did you solve it. The presentation will be discussed in the second half of the interview. you can use any tool to draw diagrams or presentation/doc .