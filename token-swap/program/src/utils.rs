use crate::curve::base::SwapResult;
use crate::curve::calculator::TradeDirection;
use crate::error::SwapError;
use crate::state::SwapState;
use std::convert::TryInto;

pub const MIN_VALUE: i128 = (-1 as i128) * ((10 as i128).pow(36));

pub fn calculate_swap_return(
    token_swap: Box<dyn SwapState>,
    in_amounts: &[u64],
    mut source_account_amount: u64,
    mut dest_account_amount: u64,
    trade_direction: TradeDirection,
) -> Vec<SwapResult> {
    let result = in_amounts
        .into_iter()
        .map(|&amount_in| {
            let res = token_swap
                .swap_curve()
                .swap(
                    to_u128(amount_in).unwrap(),
                    to_u128(source_account_amount).unwrap(),
                    to_u128(dest_account_amount).unwrap(),
                    trade_direction,
                    token_swap.fees(),
                )
                .ok_or(SwapError::ZeroTradingTokens)
                .unwrap();
            source_account_amount -= amount_in;
            dest_account_amount += amount_in;
            res
        })
        .collect();
    result
}

pub fn get_real_out_amount(distribution: &[u64], matrix: &[i128]) -> i128 {
    distribution.iter().map(|&item| matrix[item as usize]).sum()
}

// 将要兑换的数量 分成不同的深度
pub fn interpolation(in_amount: u64, partition: u64) -> Vec<u64> {
    (0..partition)
        .into_iter()
        .map(|i| {
            in_amount
                .checked_mul(i + 1)
                .expect("in_amount * i failed")
                .checked_div(partition)
                .unwrap()
        })
        .collect::<Vec<u64>>()
}

pub fn find_distribution(partition: u64, amounts: &[&[i128]]) -> Vec<u64> {
    let dex_count = amounts.len();

    let mut answer: Vec<Vec<i128>> = (0..dex_count)
        .into_iter()
        .map(|_| vec![0i128; (partition + 1) as usize])
        .collect();

    let mut parent: Vec<Vec<u64>> = (0..dex_count)
        .into_iter()
        .map(|_| vec![0u64; (partition + 1) as usize])
        .collect();

    for j in 0usize..partition as usize {
        answer[0][j] = amounts[0][j];
        for i in 1..dex_count {
            answer[i][j] = MIN_VALUE
        }
        parent[0][j] = 0;
    }
    for i in 1..dex_count {
        for j in 0usize..partition as usize {
            answer[i][j] = answer[i - 1][j];
            parent[i][j] = j as u64;
            for k in 1usize..j + 1 {
                if answer[i - 1][j - k] + amounts[i][k] > answer[i][j] {
                    answer[i][j] = answer[i - 1][j - k] + amounts[i][k];
                    parent[i][j] = (j - k) as u64;
                }
            }
        }
    }
    let mut distribution: Vec<u64> = vec![];
    let mut left = partition as usize;
    let mut dex = dex_count - 1;
    loop {
        if left <= 0 {
            break;
        }
        distribution.push(left as u64 - parent[dex][left]);
        left = parent[dex][left] as usize;
        if dex == 0 {
            break;
        }
        dex -= 1;
    }
    distribution
}

pub fn to_u128(val: u64) -> Result<u128, SwapError> {
    val.try_into().map_err(|_| SwapError::ConversionFailure)
}

#[test]
fn test() {
    let in_amount = 100;
    let partition = 1;
    let res = interpolation(in_amount, partition);
    println!("{:?}", res.as_slice());

    let aa = vec![75569i128, 0];
    let amounts = vec![aa.as_slice()];
    let res = find_distribution(partition, amounts.as_slice());
    println!("{:?}", res.as_slice());
}
